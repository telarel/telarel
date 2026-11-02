use std::borrow::Cow;
use std::sync::Arc;

use oxc::span::SourceType as OxcSourceType;

use telarel_common::{
    Ast, Language, SourceMap, SourceType, from_oxc_language,
    from_oxc_module_kind,
};

use crate::comment;
use crate::comment::adapted::NormalizedComment;
use crate::comment::raw::RawComment;
use crate::plugin::spans::{SpanMap, spans_to_bytes, spans_to_utf16};

fn debug_assert_source_anchored(ast: &Ast) {
    // Span conversion uses `Ast::source`, so the program text must match.
    debug_assert_eq!(
        ast.program().source_text,
        ast.source().as_ref(),
        "Program::source_text must stay anchored to Ast::source()"
    );
}

/// Serialize `ast` to the ESTree JSON string that the JS side receives. Byte
/// spans become UTF-16 code units, so they match JS string indices.
pub fn ast_to_estree_json(ast: &Ast) -> String {
    debug_assert_source_anchored(ast);

    let json: String = oxc_estree_codec::program_to_json(
        ast.program(),
        oxc_estree_codec::ProgramToJsonOptions::new(),
    );

    let spans: SpanMap = SpanMap::new(ast.source());

    let json: String =
        if spans.is_ascii() { json } else { spans_to_utf16(&spans, &json) };

    comment::inject(ast, &spans, json)
}

/// Rebuild an owned [`Ast`] from ESTree JSON that carries UTF-16 spans. The
/// rebuilt AST anchors to `source` and `map`, and it carries the comments from
/// the JSON.
pub fn ast_from_json(
    program_json: &str,
    source: Arc<str>,
    map: Option<SourceMap>,
    source_type: OxcSourceType,
) -> anyhow::Result<Ast> {
    let spans: SpanMap = SpanMap::new(&source);

    let raw_comments: Vec<RawComment> = comment::extract_raw(program_json)?;

    let bytes: Cow<'_, str> = if spans.is_ascii() {
        Cow::Borrowed(program_json)
    } else {
        Cow::Owned(spans_to_bytes(&spans, program_json))
    };

    let language: Language = from_oxc_language(source_type);

    let module_kind: SourceType = from_oxc_module_kind(source_type);

    let mut ast: Ast = Ast::try_from_source(
        source,
        language,
        module_kind,
        map,
        |source_text: &str, allocator: &oxc::allocator::Allocator| {
            oxc_estree_codec::json_to_program(
                bytes.as_ref(),
                oxc_estree_codec::JsonToProgramOptions {
                    allocator,
                    source_type,
                    source_text,
                },
            )
        },
    )
    .map_err(anyhow::Error::from)?;

    if !raw_comments.is_empty() {
        let (comments, starts): (Vec<NormalizedComment>, Vec<u32>) =
            comment::normalize_from_program(
                &raw_comments,
                ast.program(),
                ast.source(),
                &spans,
            )?;

        ast.with_mut(
            |allocator: &oxc::allocator::Allocator,
             program: &mut oxc::ast::ast::Program<'_>| {
                comment::attach_with_starts(
                    allocator, program, comments, &starts,
                );
            },
        );
    }

    Ok(ast)
}

/// Rebuild an owned [`Ast`] from an ESTree program value.
pub fn ast_from_program(
    program: &serde_json::Value,
    code: String,
    map: Option<SourceMap>,
    source_type: OxcSourceType,
) -> anyhow::Result<Ast> {
    ast_from_json(
        &serde_json::to_string(program)?,
        Arc::from(code),
        map,
        source_type,
    )
}

#[cfg(test)]
mod tests {
    use oxc::allocator::Allocator;
    use oxc::ast::ast::{CommentContent, Program};
    use telarel_common::{GenerateCodeOptions, generate_code};

    use super::*;
    use crate::ast::resolve::source_type_for_generate;
    use crate::ast::test_util::{fixture_ast, parse_fixture, sample_map};

    #[test]
    fn test_ast_to_estree_json_ascii_is_identity() {
        let ast: Ast = fixture_ast("const a = 1;");

        let json: String = ast_to_estree_json(&ast);

        assert!(json.contains(r#""type":"Program""#), "{json}");
        assert!(json.contains(r#""start":0"#), "{json}");
    }

    #[test]
    fn test_ast_to_estree_json_accepts_rebuilt_source_text() {
        // `with_input_map` reallocates `source_text` into a fresh arena, so the
        // anchor check must compare content, not pointers.
        let ast: Ast =
            fixture_ast("const a = 1;").with_input_map(Some(sample_map()));

        let json: String = ast_to_estree_json(&ast);

        assert!(json.contains(r#""type":"Program""#), "{json}");
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "source_text")]
    fn test_ast_to_estree_json_rejects_mismatched_source_text() {
        let ast: Ast = Ast::from_source(
            Arc::from("const a = 1;"),
            Language::TS,
            SourceType::Script,
            None,
            |source: &str, allocator: &Allocator| {
                let mut program: Program<'_> = oxc::parser::Parser::new(
                    allocator,
                    source,
                    OxcSourceType::ts(),
                )
                .parse()
                .program;

                program.source_text = allocator.alloc_str("const b = 2;");

                program
            },
        );

        let _json: String = ast_to_estree_json(&ast);
    }

    #[test]
    fn test_ast_from_json_ignores_a_comments_string_value() {
        let source: &str = "// a";
        let json: &str = concat!(
            r#"{"type":"Program","sourceType":"script","body":[],"#,
            r#""note":"comments","start":0,"end":4}"#
        );

        let ast: Ast = ast_from_json(
            json,
            Arc::from(source),
            None,
            source_type_for_generate(Language::JS),
        )
        .expect("the program decodes without a comments key");

        assert!(ast.program().comments.is_empty());
    }

    #[test]
    fn test_ast_from_json_ignores_a_nested_comments_key() {
        let source: &str = "// a";
        let json: &str = concat!(
            r#"{"type":"Program","sourceType":"script","start":0,"end":4,"#,
            r#""body":[{"type":"ExpressionStatement","start":0,"end":4,"#,
            r#""expression":{"type":"Literal","start":0,"end":4,"value":1}}],"#,
            r#""extra":{"comments":[]}}"#
        );

        let ast: Ast = ast_from_json(
            json,
            Arc::from(source),
            None,
            source_type_for_generate(Language::JS),
        )
        .expect("a nested comments key is not the top-level comments array");

        assert!(ast.program().comments.is_empty());
    }

    #[test]
    fn test_ast_from_json_finds_a_non_first_comments_key() {
        let source: &str = "// a";
        let json: &str = concat!(
            r#"{"type":"Program","sourceType":"script","body":[],"#,
            r#""start":0,"end":4,"comments":[{"type":"Line","#,
            r#""start":0,"end":4}]}"#
        );

        let ast: Ast = ast_from_json(
            json,
            Arc::from(source),
            None,
            source_type_for_generate(Language::JS),
        )
        .expect("the non-first comments key is found");

        assert_eq!(ast.program().comments.len(), 1);
        assert_eq!(ast.program().comments[0].span.start, 0);
        assert_eq!(ast.program().comments[0].span.end, 4);
    }

    #[test]
    fn test_ast_from_json_finds_an_escaped_comments_key() {
        let source: &str = "// a";
        let json: &str = concat!(
            r#"{"type":"Program","sourceType":"script","body":[],"#,
            r#""start":0,"end":4,"\u0063omments":[{"type":"Line","#,
            r#""start":0,"end":4}]}"#
        );

        let ast: Ast = ast_from_json(
            json,
            Arc::from(source),
            None,
            source_type_for_generate(Language::JS),
        )
        .expect("the escaped comments key is found");

        assert_eq!(ast.program().comments.len(), 1);
    }

    #[test]
    fn test_ast_from_json_reports_a_comment_index_error() {
        let source: &str = "// a";
        let json: &str = concat!(
            r#"{"type":"Program","sourceType":"script","body":[],"#,
            r#""start":0,"end":4,"comments":[5]}"#
        );

        let error: anyhow::Error = ast_from_json(
            json,
            Arc::from(source),
            None,
            source_type_for_generate(Language::JS),
        )
        .expect_err("the comment entry is not an object");

        assert!(error.to_string().starts_with("comment 0: "), "{error}");
    }

    #[test]
    fn test_ast_from_json_uses_shared_source_arc() {
        let source: Arc<str> = Arc::from("const a = 1;");

        let ast: Ast = ast_from_json(
            r#"{"type":"Program","sourceType":"module","body":[],"start":0,"end":0}"#,
            Arc::clone(&source),
            None,
            source_type_for_generate(Language::JS),
        )
        .expect("read-back succeeds");

        assert!(Arc::ptr_eq(ast.source(), &source));
    }

    #[test]
    fn test_ast_from_program_round_trips_ascii() {
        let ast: Ast = fixture_ast("const a = 1;");

        let json: String = ast_to_estree_json(&ast);

        let program: serde_json::Value =
            serde_json::from_str(&json).expect("serialized program parses");

        let rebuilt: Ast = ast_from_program(
            &program,
            String::from("const a = 1;"),
            None,
            ast.program().source_type,
        )
        .expect("read-back succeeds");

        assert_eq!(rebuilt.source().as_ref(), "const a = 1;");
        assert_eq!(rebuilt.program().source_text, "const a = 1;");
        assert!(rebuilt.map().is_none());
    }

    #[test]
    fn test_ast_from_program_converts_utf16_spans_to_bytes() {
        // `日` is three UTF-8 bytes but one UTF-16 code unit: a read-back that
        // forgot the conversion would misplace every span after it.
        let ast: Ast = fixture_ast("日a");

        let json: String = ast_to_estree_json(&ast);

        let program: serde_json::Value =
            serde_json::from_str(&json).expect("serialized program parses");

        let rebuilt: Ast = ast_from_program(
            &program,
            String::from("日a"),
            None,
            ast.program().source_type,
        )
        .expect("read-back succeeds");

        assert_eq!(rebuilt.source().as_ref(), "日a");

        // A correct read-back re-serializes to the exact UTF-16 JSON it came
        // from. A byte to UTF-16 mix-up would shift every span after `日`.
        assert_eq!(ast_to_estree_json(&rebuilt), json);
    }

    #[test]
    fn test_ast_from_program_attaches_returned_map() {
        let ast: Ast = fixture_ast("const a = 1;");

        let json: String = ast_to_estree_json(&ast);

        let program: serde_json::Value =
            serde_json::from_str(&json).expect("serialized program parses");

        let input_map: SourceMap = sample_map();

        let rebuilt: Ast = ast_from_program(
            &program,
            String::from("const a = 1;"),
            Some(input_map.clone()),
            ast.program().source_type,
        )
        .expect("read-back succeeds");

        let carried: &SourceMap =
            rebuilt.map().expect("the returned map is attached");

        assert_eq!(carried.to_json_string(), input_map.to_json_string());
    }

    #[test]
    fn test_comments_survive_a_nested_trailing_round_trip() {
        // The comment sits after the last statement in a function body, so its
        // next printable anchor is the program end. The text must survive even
        // though its position changes.
        let code: &str = "function f() {\n  const a = 1; // tail\n}\n";
        let ast: Ast = parse_fixture(code, Language::JS);
        let json: String = ast_to_estree_json(&ast);

        let program: serde_json::Value =
            serde_json::from_str(&json).expect("the serialized program parses");

        let rebuilt: Ast = ast_from_program(
            &program,
            code.to_string(),
            None,
            ast.program().source_type,
        )
        .expect("read-back succeeds");

        let generated: String = generate_code(
            &rebuilt,
            GenerateCodeOptions { file: String::from("index.ts") },
        )
        .code;

        assert!(generated.contains("// tail"), "{generated}");
    }

    #[test]
    fn test_comments_survive_inner_position_round_trips() {
        let cases: [(&str, &str); 7] = [
            ("function f(/* c */ a) {}", "/* c */"),
            ("const x = /* c */ 1;", "/* c */"),
            ("const x = [/* c */ 1];", "/* c */"),
            ("const { /* c */ a } = x;", "/* c */"),
            ("const [/* c */ a] = x;", "/* c */"),
            ("import { /* c */ x } from \"y\";", "/* c */"),
            ("function f(/** jsdoc */ a) {}", "/** jsdoc */"),
        ];

        for (code, expected) in cases {
            let ast: Ast = parse_fixture(code, Language::JS);
            let json: String = ast_to_estree_json(&ast);
            let program: serde_json::Value = serde_json::from_str(&json)
                .expect("the serialized program parses");

            let rebuilt: Ast = ast_from_program(
                &program,
                code.to_string(),
                None,
                ast.program().source_type,
            )
            .expect("read-back succeeds");

            let generated: String = generate_code(
                &rebuilt,
                GenerateCodeOptions { file: String::from("index.ts") },
            )
            .code;

            assert!(
                generated.contains(expected),
                "{expected} missing from {generated} for {code}"
            );
        }
    }

    #[test]
    fn test_pure_annotation_text_survives_a_json_rebuild() {
        let code: &str = "/*#__PURE__*/ f();";
        let ast: Ast = parse_fixture(code, Language::JS);
        let json: String = ast_to_estree_json(&ast);
        let program: serde_json::Value =
            serde_json::from_str(&json).expect("the serialized program parses");

        let rebuilt: Ast = ast_from_program(
            &program,
            code.to_string(),
            None,
            ast.program().source_type,
        )
        .expect("read-back succeeds");

        assert_eq!(
            rebuilt.program().comments[0].content,
            CommentContent::None,
            "the rebuild demotes the annotation to a normal comment"
        );

        let generated: String = generate_code(
            &rebuilt,
            GenerateCodeOptions { file: String::from("index.ts") },
        )
        .code;

        assert!(generated.contains("/*#__PURE__*/"), "{generated}");
    }

    #[test]
    fn test_comments_survive_an_ascii_json_round_trip() {
        let code: &str = "// line\nconst a = 1;\n/* block */\n";
        let ast: Ast = parse_fixture(code, Language::JS);

        assert!(SpanMap::new(ast.source()).is_ascii());

        let json: String = ast_to_estree_json(&ast);

        assert!(json.starts_with("{\"comments\":"), "{json}");

        let rebuilt: Ast = ast_from_json(
            &json,
            Arc::from(code),
            None,
            ast.program().source_type,
        )
        .expect("read-back succeeds");

        assert_eq!(rebuilt.program().comments.len(), 2);

        let generated: String = generate_code(
            &rebuilt,
            GenerateCodeOptions { file: String::from("index.ts") },
        )
        .code;

        for expected in ["// line", "/* block */"] {
            assert!(
                generated.contains(expected),
                "{expected} missing from {generated}"
            );
        }
    }

    #[test]
    fn test_comments_survive_a_non_ascii_json_round_trip() {
        let code: &str = "// µ\nconst a = \"日\"; /* 😀 */\n";
        let ast: Ast = parse_fixture(code, Language::JS);

        assert!(!SpanMap::new(ast.source()).is_ascii());

        let json: String = ast_to_estree_json(&ast);

        assert!(json.starts_with("{\"comments\":"), "{json}");

        let rebuilt: Ast = ast_from_json(
            &json,
            Arc::from(code),
            None,
            ast.program().source_type,
        )
        .expect("read-back succeeds");

        assert_eq!(rebuilt.program().comments.len(), 2);

        let generated: String = generate_code(
            &rebuilt,
            GenerateCodeOptions { file: String::from("index.ts") },
        )
        .code;

        for expected in ["// µ", "/* 😀 */"] {
            assert!(
                generated.contains(expected),
                "{expected} missing from {generated}"
            );
        }
    }

    #[test]
    fn test_comments_survive_estree_round_trip() {
        let code: &str = concat!(
            "// line\n",
            "const a = 1;\n",
            "/* block */\n",
            "/** jsdoc */\n",
            "function f() {}\n",
            "//! legal\n",
        );
        let ast: Ast = parse_fixture(code, Language::JS);
        let json: String = ast_to_estree_json(&ast);

        assert!(json.starts_with("{\"comments\":"), "{json}");

        let program: serde_json::Value =
            serde_json::from_str(&json).expect("the serialized program parses");

        let rebuilt: Ast = ast_from_program(
            &program,
            code.to_string(),
            None,
            ast.program().source_type,
        )
        .expect("read-back succeeds");

        assert_eq!(rebuilt.program().comments.len(), 4);

        let generated: String = generate_code(
            &rebuilt,
            GenerateCodeOptions { file: String::from("index.ts") },
        )
        .code;

        for expected in ["// line", "/* block */", "/** jsdoc */", "//! legal"]
        {
            assert!(
                generated.contains(expected),
                "{expected} missing from {generated}"
            );
        }
    }
}
