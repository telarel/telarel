use std::path::PathBuf;

use oxc::span::SourceType as OxcSourceType;

use crate::_types::grammar::language::{Language, grammar_source_type};
use crate::_types::grammar::source_type::{SourceType, with_module_kind};
use crate::_types::options::resolvable::ResolvableOptions;
use crate::_types::options::resolved::ResolvedOptions;
use crate::_types::options::source_map::SourceMapOptions;
use crate::helpers::path::{get_process_cwd, strip_file_suffix};

/// Infer the oxc source type from a file, falling back to the default
/// (an ES module) for extensionless or unknown files.
pub fn source_type_for_module_id(module_id: &str) -> OxcSourceType {
    let path: &str = strip_file_suffix(module_id);

    OxcSourceType::from_path(path).unwrap_or_default()
}

/// The oxc grammar for a file: an explicit language wins;
/// otherwise infer from the file.
pub fn resolve_grammar(
    file: &str,
    language: Option<Language>,
) -> OxcSourceType {
    match language {
        | Some(language) => grammar_source_type(language),
        | None => source_type_for_module_id(file),
    }
}

/// Narrow an oxc source type to an internal grammar.
pub fn from_oxc_language(source_type: OxcSourceType) -> Language {
    match (source_type.is_typescript(), source_type.is_jsx()) {
        | (true, true) => Language::TSX,
        | (true, false) => {
            if source_type.is_typescript_definition() {
                Language::DTS
            } else {
                Language::TS
            }
        },
        | (false, true) => Language::JSX,
        | (false, false) => Language::JS,
    }
}

/// Infer the telarel grammar for a file.
pub fn infer_language(
    file: &str,
    language: Option<Language>,
) -> Language {
    from_oxc_language(resolve_grammar(file, language))
}

/// Narrow an oxc source type to an internal module kind.
pub fn from_oxc_module_kind(source_type: OxcSourceType) -> SourceType {
    if source_type.is_script() {
        SourceType::Script
    } else if source_type.is_commonjs() {
        SourceType::CommonJS
    } else if source_type.is_module() {
        SourceType::Module
    } else {
        SourceType::Unambiguous
    }
}

/// Resolve the telarel source type for a file: the resolved grammar's module kind,
/// overlaid by an explicit option.
pub fn resolve_source_type(
    file: &str,
    language: Option<Language>,
    source_type: Option<SourceType>,
) -> SourceType {
    let grammar: OxcSourceType = resolve_grammar(file, language);

    let resolved: OxcSourceType = match source_type {
        | Some(module_kind) => with_module_kind(grammar, module_kind),
        | None => grammar,
    };

    from_oxc_module_kind(resolved)
}

/// Resolve the optional run settings into concrete options.
pub fn resolve_options(
    options: ResolvableOptions,
    plugin_names: Vec<String>,
) -> ResolvedOptions {
    let cwd: PathBuf = options.cwd.unwrap_or_else(get_process_cwd);
    let file: String = options.file.unwrap_or_else(|| String::from("index.ts"));
    let language: Language = infer_language(&file, options.language);
    let source_type: SourceType =
        resolve_source_type(&file, options.language, options.source_type);
    let source_map: SourceMapOptions = options.source_map.unwrap_or_default();

    ResolvedOptions {
        cwd,
        file,
        language,
        source_type,
        source_map,
        plugins: plugin_names,
    }
}

#[cfg(test)]
mod tests {
    use crate::_types::options::source_map::input::{
        InputSourceMap, InputSourceMapMode,
    };
    use crate::_types::options::source_map::output::{
        OutputSourceMap, OutputSourceMapMode,
    };

    use super::*;

    fn resolvable(
        file: Option<&str>,
        language: Option<Language>,
        source_type: Option<SourceType>,
    ) -> ResolvableOptions {
        ResolvableOptions {
            file: file.map(String::from),
            language,
            source_type,
            ..ResolvableOptions::default()
        }
    }

    #[test]
    fn test_from_oxc_language_narrows_grammar() {
        assert_eq!(from_oxc_language(OxcSourceType::ts()), Language::TS);
        assert_eq!(from_oxc_language(OxcSourceType::d_ts()), Language::DTS);
        assert_eq!(from_oxc_language(OxcSourceType::jsx()), Language::JSX);
        assert_eq!(from_oxc_language(OxcSourceType::tsx()), Language::TSX);
        assert_eq!(from_oxc_language(OxcSourceType::default()), Language::JS);
    }

    #[test]
    fn test_infer_language_defaults_to_typescript_for_ts_file() {
        assert_eq!(infer_language("index.ts", None), Language::TS);
    }

    #[test]
    fn test_infer_language_defaults_to_js_for_js_file() {
        assert_eq!(infer_language("index.js", None), Language::JS);
    }

    #[test]
    fn test_infer_language_explicit_wins() {
        assert_eq!(
            infer_language("index.js", Some(Language::TS)),
            Language::TS
        );
    }

    #[test]
    fn test_infer_language_tsx_detected() {
        assert_eq!(infer_language("index.tsx", None), Language::TSX);
    }

    #[test]
    fn test_infer_language_dts_detected() {
        assert_eq!(infer_language("index.d.ts", None), Language::DTS);
    }

    #[test]
    fn test_infer_language_jsx_detected() {
        assert_eq!(infer_language("index.jsx", None), Language::JSX);
    }

    #[test]
    fn test_infer_language_strips_query_suffix() {
        assert_eq!(infer_language("index.js?v=d48eb52a", None), Language::JS);
    }

    #[test]
    fn test_infer_language_strips_hash_suffix() {
        assert_eq!(infer_language("index.ts#hash", None), Language::TS);
    }

    #[test]
    fn test_infer_language_strips_query_and_hash() {
        assert_eq!(infer_language("index.tsx?v=1#h", None), Language::TSX);
    }

    #[test]
    fn test_from_oxc_module_kind_narrows_module_kind() {
        let script: OxcSourceType = OxcSourceType::default().with_script(true);
        assert_eq!(from_oxc_module_kind(script), SourceType::Script);

        let commonjs: OxcSourceType =
            OxcSourceType::default().with_commonjs(true);
        assert_eq!(from_oxc_module_kind(commonjs), SourceType::CommonJS);

        let module: OxcSourceType = OxcSourceType::default().with_module(true);
        assert_eq!(from_oxc_module_kind(module), SourceType::Module);

        assert_eq!(
            from_oxc_module_kind(OxcSourceType::unambiguous()),
            SourceType::Unambiguous,
        );
    }

    #[test]
    fn test_resolve_source_type_keeps_grammar_module_kind_without_option() {
        // An explicit language yields an unambiguous grammar: its module
        // kind is telarel's `Unambiguous`, mirrored from the parser.
        assert_eq!(
            resolve_source_type("index.ts", Some(Language::TS), None),
            SourceType::Unambiguous,
        );

        assert_eq!(
            resolve_source_type("index.ts", None, None),
            SourceType::Unambiguous,
        );

        assert_eq!(
            resolve_source_type("index.mjs", None, None),
            SourceType::Module,
        );
    }

    #[test]
    fn test_resolve_source_type_explicit_module_kind_wins() {
        assert_eq!(
            resolve_source_type("index.ts", None, Some(SourceType::Script)),
            SourceType::Script,
        );
    }

    #[test]
    fn test_resolve_source_type_cjs_extension_resolves_commonjs() {
        assert_eq!(infer_language("index.cjs", None), Language::JS);

        assert_eq!(
            resolve_source_type("index.cjs", None, None),
            SourceType::CommonJS,
        );
    }

    #[test]
    fn test_resolve_options_explicit_fields_pass_through() {
        let options: ResolvableOptions = ResolvableOptions {
            cwd: Some(PathBuf::from("/repo")),
            file: Some(String::from("src/app.tsx")),
            language: Some(Language::TSX),
            source_type: Some(SourceType::Script),
            source_map: Some(SourceMapOptions::default()),
        };

        let resolved: ResolvedOptions =
            resolve_options(options, vec![String::from("probe")]);

        assert_eq!(resolved.cwd, PathBuf::from("/repo"));
        assert_eq!(resolved.file, "src/app.tsx");
        assert_eq!(resolved.language, Language::TSX);
        assert_eq!(resolved.source_type, SourceType::Script);
        assert!(resolved.source_map.input.is_none());
        assert!(resolved.source_map.output.is_none());
        assert_eq!(resolved.plugins, vec![String::from("probe")]);
    }

    #[test]
    fn test_resolve_options_defaults_file_to_index_ts_and_infers_ts() {
        let resolved: ResolvedOptions =
            resolve_options(ResolvableOptions::default(), vec![]);

        assert_eq!(resolved.file, "index.ts");
        assert_eq!(resolved.language, Language::TS);
        assert_eq!(resolved.source_type, SourceType::Unambiguous);
    }

    #[test]
    fn test_resolve_options_explicit_language_and_source_type_win() {
        let options: ResolvableOptions = resolvable(
            Some("index.js"),
            Some(Language::TS),
            Some(SourceType::Script),
        );

        let resolved: ResolvedOptions = resolve_options(options, vec![]);

        assert_eq!(resolved.file, "index.js");
        assert_eq!(resolved.language, Language::TS);
        assert_eq!(resolved.source_type, SourceType::Script);
    }

    #[test]
    fn test_resolve_options_source_map_defaults_to_off() {
        let resolved: ResolvedOptions =
            resolve_options(ResolvableOptions::default(), vec![]);

        assert!(resolved.source_map.input.is_none());
        assert!(resolved.source_map.output.is_none());
    }

    #[test]
    fn test_resolve_options_keeps_explicit_source_map() {
        let source_map: SourceMapOptions = SourceMapOptions {
            input: Some(InputSourceMap::Mode(InputSourceMapMode::Inline)),
            output: Some(OutputSourceMap::Mode(OutputSourceMapMode::Both)),
        };

        let options: ResolvableOptions = ResolvableOptions {
            source_map: Some(source_map),
            ..ResolvableOptions::default()
        };

        let resolved: ResolvedOptions = resolve_options(options, vec![]);

        assert!(matches!(
            resolved.source_map.input,
            Some(InputSourceMap::Mode(InputSourceMapMode::Inline)),
        ));
        assert!(matches!(
            resolved.source_map.output,
            Some(OutputSourceMap::Mode(OutputSourceMapMode::Both)),
        ));
    }

    #[test]
    fn test_resolve_options_cwd_defaults_to_process_cwd() {
        let resolved: ResolvedOptions =
            resolve_options(ResolvableOptions::default(), vec![]);

        let expected: PathBuf =
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));

        assert_eq!(resolved.cwd, expected);
    }

    #[test]
    fn test_resolve_options_keeps_plugins_in_order_with_duplicates() {
        let resolved: ResolvedOptions = resolve_options(
            ResolvableOptions::default(),
            vec![
                String::from("first"),
                String::from("second"),
                String::from("first"),
            ],
        );

        assert_eq!(resolved.plugins, vec!["first", "second", "first"]);
    }
}
