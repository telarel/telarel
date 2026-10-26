//! The single source of truth for `Ast` ⇄ JS conversions.
//!
//! Every NAPI function and the plugin bridge use this module. The module
//! converts between the internal [`Ast`] and the JS-facing normalized form
//! (`{ program, code, map?, language }`). The ESTree spans that cross the
//! boundary are UTF-16 code units (JS string indices). The internal [`Ast`]
//! keeps byte spans.
//!
//! The caller always computes the `SourceType` of a decoded program and
//! passes it in:
//!
//! - [`source_type_for_options`] mirrors the pipeline's explicit resolution:
//!   the grammar from `file` or `language`, overlaid with an explicit module
//!   kind.
//! - [`source_type_for_generate`] keeps the module kind unambiguous, so the
//!   codec resolves it from the program JSON's `sourceType`. This is the
//!   generate path, where only the grammar comes from options, AST, or file.
//! - [`source_type_for_ast_input`] applies the AST-entry precedence: an
//!   explicit concrete kind, then the program's `sourceType`, then the file
//!   extension.

use std::sync::Arc;

use oxc::span::SourceType as OxcSourceType;

use telarel_common::{
    Ast, Language, SourceMap, SourceType, from_oxc_language,
    from_oxc_module_kind, grammar_source_type, resolve_grammar,
    resolve_source_type, with_module_kind,
};

use crate::_types::options::parse_language_option;
use crate::_types::sourcemap::JsSourceMap;
use crate::comment;
use crate::comment::adapted::NormalizedComment;
use crate::plugin::spans::{SpanMap, spans_to_bytes, spans_to_utf16};

/// The JS-facing normalized AST: the ESTree `program`, its `code`, its input
/// `map` as a v3 source-map object, and its grammar label.
pub struct JsAst {
    /// The ESTree program.
    pub program: serde_json::Value,
    /// The source text the program spans anchor to.
    pub code: String,
    /// The input source map, when the AST carries one.
    pub map: Option<JsSourceMap>,
    /// The grammar of the program, as its canonical label.
    pub language: String,
}

/// The JS-facing input form of the normalized AST. This is the read side of
/// [`JsAst`].
pub struct JsAstInput {
    /// The ESTree program.
    pub program: serde_json::Value,
    /// The source text the program spans anchor to.
    pub code: String,
    /// The source map object for the input, when the caller supplied one.
    pub map: Option<serde_json::Value>,
    /// The grammar of the program. Absent means unset.
    pub language: Option<String>,
}

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

    let json: String = spans_to_utf16(&spans, &json);

    comment::inject(ast, &spans, json)
}

/// Convert an input source map into its JS-facing v3 object.
pub fn source_map_to_js(map: &SourceMap) -> JsSourceMap {
    JsSourceMap::from(map.clone())
}

/// Build the normalized AST object for the JS side from `ast`.
pub fn ast_to_js(ast: &Ast) -> anyhow::Result<JsAst> {
    let json: String = ast_to_estree_json(ast);

    Ok(JsAst {
        program: serde_json::from_str(&json)?,
        code: ast.source().to_string(),
        map: ast.map().map(source_map_to_js),
        language: ast.language().as_str().to_string(),
    })
}

/// Rebuild an owned [`SourceMap`] from its JS-facing v3 object.
pub fn js_source_map_to_owned(map: &JsSourceMap) -> anyhow::Result<SourceMap> {
    let json: oxc_sourcemap::JSONSourceMap = oxc_sourcemap::JSONSourceMap {
        version: map.version,
        file: map.file.clone(),
        mappings: map.mappings.clone(),
        source_root: map.source_root.clone(),
        sources: map.sources.clone(),
        sources_content: map.sources_content.clone(),
        names: map.names.clone(),
        debug_id: None,
        ignore_list: map.ignore_list.clone(),
    };

    oxc_sourcemap::SourceMap::from_json(json).map_err(anyhow::Error::msg)
}

/// Parse a source map that a hook sends as a JSON string into an owned map.
pub fn parse_json_map(json: &str) -> anyhow::Result<SourceMap> {
    Ok(oxc_sourcemap::OwnedSourceMap::from_json_string(json)
        .map_err(anyhow::Error::msg)?
        .into_inner())
}

/// Parse a JS source-map object into an owned map.
pub fn parse_map_value(value: &serde_json::Value) -> anyhow::Result<SourceMap> {
    parse_json_map(&serde_json::to_string(value)?)
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

    let comments: Vec<NormalizedComment> =
        comment::extract(program_json, &source, &spans)?;

    let bytes: String = spans_to_bytes(&spans, program_json);

    let language: Language = from_oxc_language(source_type);
    let module_kind: SourceType = from_oxc_module_kind(source_type);

    let mut ast: Ast = Ast::try_from_source(
        source,
        language,
        module_kind,
        map,
        |source_text: &str, allocator: &oxc::allocator::Allocator| {
            oxc_estree_codec::json_to_program(
                &bytes,
                oxc_estree_codec::JsonToProgramOptions {
                    allocator,
                    source_type,
                    source_text,
                },
            )
        },
    )
    .map_err(anyhow::Error::from)?;

    if !comments.is_empty() {
        ast.with_mut(
            |allocator: &oxc::allocator::Allocator,
             program: &mut oxc::ast::ast::Program<'_>| {
                comment::attach(allocator, program, comments);
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

/// Rebuild an owned [`Ast`] from the normalized `{ program, code, map? }`
/// input.
///
/// The caller computes `source_type`. Use [`source_type_for_options`] for an
/// explicit source type, [`source_type_for_generate`] when the program JSON
/// owns the module kind, or [`source_type_for_ast_input`] for the AST-entry
/// precedence used by `rewrite` and `compileFromAst`.
pub fn js_to_ast(
    input: JsAstInput,
    source_type: OxcSourceType,
) -> anyhow::Result<Ast> {
    let map: Option<SourceMap> = match &input.map {
        | Some(value) => Some(parse_map_value(value)?),
        | None => None,
    };

    ast_from_program(&input.program, input.code, map, source_type)
}

/// The explicit source type: the grammar resolved from `file` or `language`,
/// overlaid with an explicit module kind.
pub fn source_type_for_options(
    file: &str,
    language: Option<Language>,
    module_kind: SourceType,
) -> OxcSourceType {
    with_module_kind(resolve_grammar(file, language), module_kind)
}

/// The generate-path source type: the caller-resolved grammar with an
/// unambiguous module kind, so the codec resolves the module kind from the
/// program JSON's `sourceType`.
pub fn source_type_for_generate(language: Language) -> OxcSourceType {
    with_module_kind(grammar_source_type(language), SourceType::Unambiguous)
}

fn program_states_source_type(program: &serde_json::Value) -> bool {
    program.get("sourceType").and_then(serde_json::Value::as_str).is_some()
}

/// The AST-entry source type: an explicit concrete module kind, then the
/// program's own `sourceType`, then the file extension.
///
/// Must stay aligned with the module-kind resolution in `telarel_core`'s pipeline.
pub fn source_type_for_ast_input(
    file: &str,
    language: Language,
    explicit: Option<SourceType>,
    program: &serde_json::Value,
) -> OxcSourceType {
    match explicit {
        | Some(source_type) if source_type != SourceType::Unambiguous => {
            source_type_for_options(file, Some(language), source_type)
        },
        | _ if program_states_source_type(program) => {
            source_type_for_generate(language)
        },
        | _ => {
            let file_kind: SourceType = resolve_source_type(file, None, None);

            source_type_for_options(file, Some(language), file_kind)
        },
    }
}

/// Parse the optional grammar label that an inbound JS AST carries.
///
/// An absent label stays `None`. The caller applies the precedence fallbacks.
pub fn parse_ast_language(
    language: Option<&str>
) -> napi::Result<Option<Language>> {
    parse_language_option(language)
}

/// Resolve the effective grammar: explicit options, then AST, then file.
///
/// The caller passes `file_language` already inferred from the resolved
/// options or file, so the last rung keeps extension inference as the
/// fallback.
pub fn effective_language(
    explicit: Option<Language>,
    ast: Option<Language>,
    file_language: Language,
) -> Language {
    explicit.or(ast).unwrap_or(file_language)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use oxc::allocator::Allocator;
    use oxc::ast::ast::{CommentContent, Program};
    use telarel_common::{
        GenerateCodeOptions, ParseAstOptions, generate_code, parse_ast,
    };

    use super::*;

    fn fixture_ast(code: &str) -> Ast {
        parse_ast(
            code.to_owned(),
            ParseAstOptions {
                cwd: PathBuf::from("/repo"),
                file: String::from("a.ts"),
                language: None,
                source_type: None,
                input_map: None,
            },
        )
        .expect("fixture parses")
    }

    fn parse_fixture(
        code: &str,
        language: Language,
    ) -> Ast {
        parse_ast(
            code.to_owned(),
            ParseAstOptions {
                cwd: PathBuf::from("/repo"),
                file: String::from("index.ts"),
                language: Some(language),
                source_type: None,
                input_map: None,
            },
        )
        .expect("fixture parses")
    }

    fn sample_map() -> SourceMap {
        let mut builder: oxc_sourcemap::SourceMapBuilder<'_> =
            oxc_sourcemap::SourceMapBuilder::default();

        builder.set_file("upstream.ts");

        let source_id: u32 =
            builder.add_source_and_content("upstream.ts", "const a = 1;");

        builder.add_token(0, 0, 0, 0, Some(source_id), None);

        builder.into_owned_sourcemap().into_inner()
    }

    /// Decode a minimal program carrying `sourceType` under `source_type`.
    fn decode(
        source_type_label: &str,
        source_type: OxcSourceType,
    ) -> Ast {
        let program: serde_json::Value = serde_json::json!({
            "type": "Program",
            "sourceType": source_type_label,
            "body": [],
            "start": 0,
            "end": 0,
        });

        ast_from_program(&program, String::new(), None, source_type)
            .expect("the program decodes")
    }

    fn program_with_source_type(
        source_type_label: Option<&str>
    ) -> serde_json::Value {
        let mut program: serde_json::Value = serde_json::json!({
            "type": "Program",
            "body": [],
            "start": 0,
            "end": 0,
        });

        if let Some(source_type_label) = source_type_label {
            program["sourceType"] =
                serde_json::Value::String(source_type_label.to_string());
        }

        program
    }

    fn decode_ast_input(
        file: &str,
        language: Language,
        explicit: Option<SourceType>,
        source_type_label: Option<&str>,
    ) -> Ast {
        let program: serde_json::Value =
            program_with_source_type(source_type_label);

        let source_type: OxcSourceType =
            source_type_for_ast_input(file, language, explicit, &program);

        ast_from_program(&program, String::new(), None, source_type)
            .expect("the program decodes")
    }

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
    fn test_ast_to_js_carries_program_and_code() {
        let ast: Ast = fixture_ast("const a = 1;");

        let js: JsAst = ast_to_js(&ast).expect("the AST converts");

        assert_eq!(js.program["type"], "Program");
        assert_eq!(js.code, "const a = 1;");
        assert!(js.map.is_none());
    }

    #[test]
    fn test_ast_to_js_carries_input_map() {
        let ast: Ast =
            fixture_ast("const a = 1;").with_input_map(Some(sample_map()));

        let js: JsAst = ast_to_js(&ast).expect("the AST converts");

        let map: JsSourceMap = js.map.expect("the input map is carried");

        assert_eq!(map.sources, vec![String::from("upstream.ts")]);
    }

    #[test]
    fn test_js_source_map_to_owned_round_trips() {
        let map: SourceMap = sample_map();

        let js: JsSourceMap = source_map_to_js(&map);

        let rebuilt: SourceMap =
            js_source_map_to_owned(&js).expect("the map rebuilds");

        assert_eq!(rebuilt.to_json_string(), map.to_json_string());
    }

    #[test]
    fn test_parse_json_map_round_trips() {
        let map: SourceMap = sample_map();

        let parsed: SourceMap =
            parse_json_map(&map.to_json_string()).expect("map parses");

        assert_eq!(parsed.to_json_string(), map.to_json_string());
    }

    #[test]
    fn test_parse_json_map_invalid_errors() {
        assert!(parse_json_map("not json").is_err());
    }

    #[test]
    fn test_parse_map_value_round_trips() {
        let map: SourceMap = sample_map();

        let value: serde_json::Value =
            serde_json::from_str(&map.to_json_string())
                .expect("serialized map parses");

        let parsed: SourceMap =
            parse_map_value(&value).expect("the map object parses");

        assert_eq!(parsed.to_json_string(), map.to_json_string());
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
    fn test_js_to_ast_round_trips_the_out_object() {
        let ast: Ast = fixture_ast("const a = 1;");

        let js: JsAst = ast_to_js(&ast).expect("the AST converts");

        let input: JsAstInput = JsAstInput {
            program: js.program,
            code: js.code,
            map: None,
            language: Some(js.language),
        };

        let rebuilt: Ast = js_to_ast(input, ast.program().source_type)
            .expect("read-back succeeds");

        assert_eq!(rebuilt.source().as_ref(), "const a = 1;");
        assert_eq!(ast_to_estree_json(&rebuilt), ast_to_estree_json(&ast));
    }

    #[test]
    fn test_js_to_ast_parses_an_object_map() {
        let ast: Ast = fixture_ast("const a = 1;");

        let program: serde_json::Value =
            serde_json::from_str(&ast_to_estree_json(&ast))
                .expect("serialized program parses");

        let map: serde_json::Value =
            serde_json::from_str(&sample_map().to_json_string())
                .expect("serialized map parses");

        let input: JsAstInput = JsAstInput {
            program,
            code: String::from("const a = 1;"),
            map: Some(map),
            language: None,
        };

        let rebuilt: Ast = js_to_ast(input, ast.program().source_type)
            .expect("read-back succeeds");

        assert!(rebuilt.map().is_some());
    }

    #[test]
    fn test_js_to_ast_invalid_map_errors() {
        let input: JsAstInput = JsAstInput {
            program: serde_json::json!({
                "type": "Program",
                "sourceType": "module",
                "body": [],
                "start": 0,
                "end": 0,
            }),
            code: String::new(),
            map: Some(serde_json::Value::String(String::from("not json"))),
            language: None,
        };

        assert!(
            js_to_ast(input, source_type_for_generate(Language::JS)).is_err()
        );
    }

    #[test]
    fn test_explicit_source_type_wins_over_program_json() {
        // The codec passes a non-unambiguous explicit source type through
        // verbatim, which is why the generate path needs the unambiguous
        // helper.
        let source_type: OxcSourceType = source_type_for_options(
            "a.js",
            Some(Language::JS),
            SourceType::Script,
        );

        assert!(!source_type.is_unambiguous());

        let ast: Ast = decode("module", source_type);

        assert_eq!(ast.source_type(), SourceType::Script);
        assert!(ast.program().source_type.is_script());
    }

    #[test]
    fn test_explicit_source_type_infers_grammar_from_file() {
        let source_type: OxcSourceType =
            source_type_for_options("a.ts", None, SourceType::Module);

        assert!(source_type.is_typescript());
        assert!(source_type.is_module());
    }

    // An unambiguous type lets the codec read `program.sourceType`. The coarse
    // `Ast::source_type()` then reports the resolved module kind.

    #[test]
    fn test_generate_source_type_reads_module_from_program_json() {
        let ast: Ast = decode("module", source_type_for_generate(Language::JS));

        let resolved: OxcSourceType = ast.program().source_type;

        assert!(resolved.is_javascript(), "{resolved:?}");
        assert!(resolved.is_module(), "{resolved:?}");
    }

    #[test]
    fn test_generate_source_type_reads_script_from_program_json() {
        let ast: Ast = decode("script", source_type_for_generate(Language::JS));

        let resolved: OxcSourceType = ast.program().source_type;

        assert!(resolved.is_script(), "{resolved:?}");
    }

    #[test]
    fn test_generate_source_type_reads_commonjs_from_program_json() {
        let ast: Ast =
            decode("commonjs", source_type_for_generate(Language::JS));

        let resolved: OxcSourceType = ast.program().source_type;

        assert!(resolved.is_commonjs(), "{resolved:?}");
    }

    #[test]
    fn test_generate_source_type_ts_program_sanity() {
        let source_type: OxcSourceType = source_type_for_generate(Language::TS);

        assert!(source_type.is_typescript());
        assert!(source_type.is_unambiguous());

        let ast: Ast = decode("module", source_type);

        let resolved: OxcSourceType = ast.program().source_type;

        assert!(resolved.is_typescript(), "{resolved:?}");
        assert!(resolved.is_module(), "{resolved:?}");
    }

    #[test]
    fn test_generate_source_type_dts_overrides_its_module_kind() {
        // `d_ts()` is an explicit module. The generate path must still defer
        // the module kind to the program JSON.
        let source_type: OxcSourceType =
            source_type_for_generate(Language::DTS);

        assert!(source_type.is_unambiguous(), "{source_type:?}");

        let ast: Ast = decode("script", source_type);

        let resolved: OxcSourceType = ast.program().source_type;

        assert!(resolved.is_typescript_definition(), "{resolved:?}");
        assert!(resolved.is_script(), "{resolved:?}");
    }

    #[test]
    fn test_ast_input_explicit_source_type_wins() {
        let ast: Ast = decode_ast_input(
            "index.mjs",
            Language::JS,
            Some(SourceType::Script),
            Some("module"),
        );

        assert_eq!(ast.source_type(), SourceType::Script);
        assert!(ast.program().source_type.is_script());
    }

    #[test]
    fn test_ast_input_program_source_type_wins_over_cjs_file() {
        let ast: Ast =
            decode_ast_input("index.cjs", Language::JS, None, Some("module"));

        assert_eq!(ast.source_type(), SourceType::Module);
        assert!(ast.program().source_type.is_module());
    }

    #[test]
    fn test_ast_input_program_source_type_wins_over_mjs_file() {
        let ast: Ast =
            decode_ast_input("index.mjs", Language::JS, None, Some("commonjs"));

        assert_eq!(ast.source_type(), SourceType::CommonJS);
        assert!(ast.program().source_type.is_commonjs());
    }

    #[test]
    fn test_ast_input_unambiguous_defers_to_program_source_type() {
        let ast: Ast = decode_ast_input(
            "index.cjs",
            Language::JS,
            Some(SourceType::Unambiguous),
            Some("module"),
        );

        assert_eq!(ast.source_type(), SourceType::Module);
        assert!(ast.program().source_type.is_module());
    }

    #[test]
    fn test_ast_input_falls_back_to_file_without_program_source_type() {
        let ast: Ast = decode_ast_input("index.cjs", Language::JS, None, None);

        assert_eq!(ast.source_type(), SourceType::CommonJS);
        assert!(ast.program().source_type.is_commonjs());
    }

    #[test]
    fn test_ast_input_unambiguous_falls_back_to_file_without_program_kind() {
        let ast: Ast = decode_ast_input(
            "index.cjs",
            Language::JS,
            Some(SourceType::Unambiguous),
            None,
        );

        assert_eq!(ast.source_type(), SourceType::CommonJS);
        assert!(ast.program().source_type.is_commonjs());
    }

    #[test]
    fn test_ast_source_type_reports_the_resolved_program_module_kind() {
        // The coarse getter follows the module kind that the codec resolved
        // into the program. The raw oxc type stays readable through
        // `program()`.
        let passed: OxcSourceType = source_type_for_generate(Language::JS);

        let ast: Ast = decode("module", passed);

        assert_eq!(ast.source_type(), SourceType::Module);
        assert!(ast.program().source_type.is_module());

        let explicit_script: OxcSourceType =
            source_type_for_options("a.ts", None, SourceType::Script);

        let script: Ast = decode("module", explicit_script);

        assert_eq!(script.source_type(), SourceType::Script);
        assert!(script.program().source_type.is_script());
    }

    #[test]
    fn test_ast_to_js_emits_language_for_every_grammar() {
        let cases: [(Language, &str); 5] = [
            (Language::JS, "const a = 1;"),
            (Language::TS, "const a: number = 1;"),
            (Language::DTS, "declare const a: string;"),
            (Language::JSX, "const x = <div />;"),
            (Language::TSX, "const x: JSX.Element = <div />;"),
        ];

        for (language, code) in cases {
            let ast: Ast = parse_fixture(code, language);

            let js: JsAst = ast_to_js(&ast).expect("the AST converts");

            assert_eq!(js.language, language.as_str(), "{language:?}");
        }
    }

    #[test]
    fn test_js_ast_input_round_trips_language() {
        let ast: Ast = parse_fixture("const x = <div />;", Language::JSX);

        let js: JsAst = ast_to_js(&ast).expect("the AST converts");

        let input: JsAstInput = JsAstInput {
            program: js.program.clone(),
            code: js.code.clone(),
            map: None,
            language: Some(js.language.clone()),
        };

        let outbound: crate::_types::ast::JsAst = js.into();

        assert_eq!(outbound.language, "jsx");

        let inbound: crate::_types::ast::JsAstInput =
            crate::_types::ast::JsAstInput {
                program: input.program,
                code: input.code,
                map: None,
                language: input.language,
            };

        let input: JsAstInput = inbound.into();

        assert_eq!(input.language.as_deref(), Some("jsx"));

        let rebuilt: Ast =
            js_to_ast(input, source_type_for_generate(Language::JSX))
                .expect("read-back succeeds");

        assert_eq!(rebuilt.language(), Language::JSX);
    }

    #[test]
    fn test_js_ast_input_absent_language_is_unset() {
        let inbound: crate::_types::ast::JsAstInput =
            crate::_types::ast::JsAstInput {
                program: serde_json::json!({
                    "type": "Program",
                    "sourceType": "module",
                    "body": [],
                    "start": 0,
                    "end": 0,
                }),
                code: String::new(),
                map: None,
                language: None,
            };

        let input: JsAstInput = inbound.into();

        assert!(input.language.is_none());

        let rebuilt: Ast =
            js_to_ast(input, source_type_for_generate(Language::JS))
                .expect("read-back succeeds");

        assert_eq!(rebuilt.language(), Language::JS);
    }

    #[test]
    fn test_effective_language_orders_explicit_ast_then_file() {
        let resolved: telarel_common::ResolvedOptions =
            telarel_common::resolve_options(
                telarel_common::ResolvableOptions::default(),
                Vec::new(),
            );

        assert_eq!(resolved.language, Language::TS, "index.ts inference");

        assert_eq!(
            effective_language(
                Some(Language::JS),
                Some(Language::JSX),
                resolved.language,
            ),
            Language::JS
        );

        assert_eq!(
            effective_language(None, Some(Language::JSX), resolved.language),
            Language::JSX
        );

        assert_eq!(
            effective_language(None, None, resolved.language),
            Language::TS
        );
    }

    #[test]
    fn test_parse_ast_language_labels() {
        let cases: [(&str, Language); 5] = [
            ("js", Language::JS),
            ("ts", Language::TS),
            ("dts", Language::DTS),
            ("jsx", Language::JSX),
            ("tsx", Language::TSX),
        ];

        for (text, language) in cases {
            assert_eq!(
                parse_ast_language(Some(text)).expect("the label parses"),
                Some(language),
                "{text}"
            );
        }

        assert_eq!(parse_ast_language(None).expect("absent is unset"), None);

        let error: napi::Error =
            parse_ast_language(Some("json")).expect_err("invalid labels error");

        assert_eq!(error.status, napi::Status::InvalidArg);
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

    #[test]
    fn test_source_type_for_generate_grammar_for_every_language() {
        let cases: [(Language, bool, bool, bool); 5] = [
            (Language::JS, false, false, false),
            (Language::TS, true, false, false),
            (Language::DTS, true, false, true),
            (Language::JSX, false, true, false),
            (Language::TSX, true, true, false),
        ];

        for (language, typescript, jsx, definition) in cases {
            let source_type: OxcSourceType = source_type_for_generate(language);

            assert_eq!(source_type.is_typescript(), typescript, "{language:?}");
            assert_eq!(source_type.is_jsx(), jsx, "{language:?}");
            assert_eq!(
                source_type.is_typescript_definition(),
                definition,
                "{language:?}"
            );
            assert!(source_type.is_unambiguous(), "{language:?}");
        }
    }
}
