use oxc::span::SourceType as OxcSourceType;

use telarel_common::{Ast, SourceMap};

use crate::_types::sourcemap::JsSourceMap;
use crate::ast::estree::{ast_from_program, ast_to_estree_json};
use crate::ast::sourcemap::{parse_map_value, source_map_to_js};

/// The JS-facing normalized AST: the ESTree `program`, its `code`, its input
/// `map` as a v3 source-map object, and its grammar label.
pub struct JsAst {
    /// The ESTree program.
    pub program: serde_json::Value,
    /// The source text that the program's spans anchor to.
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
    /// The source text that the program's spans anchor to.
    pub code: String,
    /// The source map object for the input, when the caller supplied one.
    pub map: Option<serde_json::Value>,
    /// The grammar of the program. Absent means unset.
    pub language: Option<String>,
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

/// Rebuild an owned [`Ast`] from the normalized `{ program, code, map? }`
/// input.
///
/// The caller computes `source_type`. Use
/// [`source_type_for_options`](crate::ast::resolve::source_type_for_options)
/// for an explicit source type,
/// [`source_type_for_generate`](crate::ast::resolve::source_type_for_generate)
/// when the program JSON owns the module kind, or
/// [`source_type_for_ast_input`](crate::ast::resolve::source_type_for_ast_input)
/// for the AST-entry precedence used by `rewrite` and `compileFromAst`.
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

#[cfg(test)]
mod tests {
    use telarel_common::Language;

    use super::*;
    use crate::ast::resolve::source_type_for_generate;
    use crate::ast::test_util::{fixture_ast, parse_fixture, sample_map};

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
}
