//! The single source of truth for `Ast` ⇄ JS conversions.
//!
//! Every NAPI function and the plugin bridge convert between the internal
//! [`Ast`] and the JS-facing normalized form (`{ program, code, map? }`)
//! through this module. The ESTree spans crossing the boundary are UTF-16 code
//! units (JS string indices); the internal [`Ast`] keeps byte spans.
//!
//! The `SourceType` of a decoded program is always computed by the caller and
//! passed in:
//!
//! - [`source_type_for_options`] mirrors the pipeline's explicit resolution
//!   (grammar from `file`/`language`, overlaid with an explicit module kind).
//! - [`source_type_for_generate`] keeps the module kind unambiguous, so the
//!   codec resolves it from the program JSON's `sourceType` (the generate path,
//!   whose options carry no `language`/`sourceType`).

use std::sync::Arc;

use oxc::span::SourceType as OxcSourceType;

use telarel_common::{
    Ast, Language, SourceMap, SourceType, grammar_source_type, resolve_grammar,
    with_module_kind,
};

use crate::_types::sourcemap::JsSourceMap;
use crate::plugin::spans::{SpanMap, spans_to_bytes, spans_to_utf16};

/// The JS-facing normalized AST: the ESTree `program`, its `code`, and its
/// input `map` as a v3 source-map object.
pub struct JsAst {
    /// The ESTree program.
    pub program: serde_json::Value,
    /// The source text the program spans anchor to.
    pub code: String,
    /// The input source map, when the AST carries one.
    pub map: Option<JsSourceMap>,
}

/// The JS-facing normalized AST input, the read side of [`JsAst`].
pub struct JsAstInput {
    /// The ESTree program.
    pub program: serde_json::Value,
    /// The source text the program spans anchor to.
    pub code: String,
    /// The input source map object, when the caller supplied one.
    pub map: Option<serde_json::Value>,
}

/// Serialize `ast` to the ESTree JSON string the JS side receives: byte spans
/// are rewritten to UTF-16 code units so they match JS string indices.
pub fn ast_to_estree_json(ast: &Ast) -> String {
    let json: String = oxc_estree_codec::program_to_json(
        ast.program(),
        oxc_estree_codec::ProgramToJsonOptions::new(),
    );

    let spans: SpanMap = SpanMap::new(ast.source());

    spans_to_utf16(&spans, &json)
}

/// Convert an input source map into its JS-facing v3 object.
pub fn source_map_to_js(map: &SourceMap) -> JsSourceMap {
    JsSourceMap::from(map.clone())
}

/// Build the JS-facing normalized AST object from `ast`.
pub fn ast_to_js(ast: &Ast) -> anyhow::Result<JsAst> {
    let json: String = ast_to_estree_json(ast);

    Ok(JsAst {
        program: serde_json::from_str(&json)?,
        code: ast.source().to_string(),
        map: ast.map().map(source_map_to_js),
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

/// Parse a hook's JSON-string source map into an owned map.
pub fn parse_json_map(json: &str) -> anyhow::Result<SourceMap> {
    Ok(oxc_sourcemap::OwnedSourceMap::from_json_string(json)
        .map_err(anyhow::Error::msg)?
        .into_inner())
}

/// Parse a JS source-map object into an owned map.
pub fn parse_map_value(value: &serde_json::Value) -> anyhow::Result<SourceMap> {
    parse_json_map(&serde_json::to_string(value)?)
}

/// Rebuild an owned [`Ast`] from a UTF-16-spanned ESTree program JSON string.
///
/// The spans are mapped back to byte offsets against `source` before the codec
/// rebuilds the program into a fresh arena.
pub fn ast_from_json(
    program_json: &str,
    source: Arc<str>,
    map: Option<SourceMap>,
    source_type: OxcSourceType,
) -> anyhow::Result<Ast> {
    let spans: SpanMap = SpanMap::new(&source);

    let bytes: String = spans_to_bytes(&spans, program_json);

    Ast::try_from_source(
        source,
        source_type,
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
    .map_err(anyhow::Error::from)
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
/// `source_type` is computed by the caller: [`source_type_for_options`] for
/// parse/rewrite/compile-from-ast, or [`source_type_for_generate`] for the
/// generate path, whose unambiguous module kind lets the codec resolve it from
/// `program.sourceType`.
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

/// The explicit source type: the grammar resolved from `file`/`language`,
/// overlaid with an explicit module kind.
///
/// Mirrors the plugin bridge's parse read-back; used by parse, rewrite,
/// compile-from-ast and `builtinParse`.
pub fn source_type_for_options(
    file: &str,
    language: Option<Language>,
    module_kind: SourceType,
) -> OxcSourceType {
    with_module_kind(resolve_grammar(file, language), module_kind)
}

/// The generate-path source type: the caller-resolved grammar with an
/// UNAMBIGUOUS module kind, so the codec resolves the module kind from the
/// program JSON's `sourceType`.
pub fn source_type_for_generate(language: Language) -> OxcSourceType {
    with_module_kind(grammar_source_type(language), SourceType::Unambiguous)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use telarel_common::{ParseAstOptions, parse_ast};

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

    #[test]
    fn test_ast_to_estree_json_ascii_is_identity() {
        let ast: Ast = fixture_ast("const a = 1;");

        let json: String = ast_to_estree_json(&ast);

        assert!(json.contains(r#""type":"Program""#), "{json}");
        assert!(json.contains(r#""start":0"#), "{json}");
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
            ast.source_type(),
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
            ast.source_type(),
        )
        .expect("read-back succeeds");

        assert_eq!(rebuilt.source().as_ref(), "日a");

        // A correct read-back re-serializes to the exact UTF-16 JSON it came
        // from; a byte/UTF-16 mix-up would shift every span after `日`.
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
            ast.source_type(),
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

        let input: JsAstInput =
            JsAstInput { program: js.program, code: js.code, map: None };

        let rebuilt: Ast =
            js_to_ast(input, ast.source_type()).expect("read-back succeeds");

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
        };

        let rebuilt: Ast =
            js_to_ast(input, ast.source_type()).expect("read-back succeeds");

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
        };

        assert!(
            js_to_ast(input, source_type_for_generate(Language::JS)).is_err()
        );
    }

    #[test]
    fn test_explicit_source_type_wins_over_program_json() {
        // A non-unambiguous explicit source type is passed through verbatim,
        // which is why the generate path needs the unambiguous helper.
        let source_type: OxcSourceType = source_type_for_options(
            "a.js",
            Some(Language::JS),
            SourceType::Script,
        );

        assert!(!source_type.is_unambiguous());

        let ast: Ast = decode("module", source_type);

        assert!(ast.source_type().is_script(), "{:?}", ast.source_type());
        assert!(ast.program().source_type.is_script());
    }

    #[test]
    fn test_explicit_source_type_infers_grammar_from_file() {
        let source_type: OxcSourceType =
            source_type_for_options("a.ts", None, SourceType::Module);

        assert!(source_type.is_typescript());
        assert!(source_type.is_module());
    }

    // An unambiguous type lets the codec read `program.sourceType`;
    // `Ast::source_type()` echoes the caller-passed type verbatim.

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
        // `d_ts()` is an explicit module; the generate path must still defer
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
    fn test_ast_source_type_is_the_caller_passed_type() {
        // Documents the split: the codec resolves the module kind into the
        // program, but `Ast::source_type()` echoes exactly what the caller
        // passed (here, unambiguous).
        let passed: OxcSourceType = source_type_for_generate(Language::JS);

        let ast: Ast = decode("module", passed);

        assert!(ast.source_type().is_unambiguous(), "{:?}", ast.source_type());
        assert!(ast.program().source_type.is_module());
    }
}
