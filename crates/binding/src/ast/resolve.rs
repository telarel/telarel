use oxc::span::SourceType as OxcSourceType;

use telarel_common::{
    Language, SourceType, grammar_source_type, resolve_grammar,
    resolve_source_type, with_module_kind,
};

use crate::_types::options::parse_language_option;

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
    use telarel_common::Ast;

    use super::*;
    use crate::ast::estree::ast_from_program;

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
