use oxc::span::SourceType as OxcSourceType;

use crate::_types::grammar::language::{Language, grammar_source_type};
use crate::_types::grammar::source_type::{SourceType, with_module_kind};

/// Drop the `?query`/`#hash` suffix from a module id.
fn strip_id_suffix(module_id: &str) -> &str {
    let without_hash: &str =
        module_id.split_once('#').map_or(module_id, |(path, _)| path);

    without_hash.split_once('?').map_or(without_hash, |(path, _)| path)
}

/// Infer the oxc source type from a module id, falling back to the default
/// (an ES module) for extensionless or unknown virtual files.
pub fn source_type_for_module_id(module_id: &str) -> OxcSourceType {
    let path: &str = strip_id_suffix(module_id);

    OxcSourceType::from_path(path).unwrap_or_default()
}

/// The oxc grammar for a file: an explicit language wins; otherwise infer
/// from the module id.
pub fn resolve_grammar(
    file: &str,
    language: Option<Language>,
) -> OxcSourceType {
    match language {
        | Some(language) => grammar_source_type(language),
        | None => source_type_for_module_id(file),
    }
}

/// Infer the telarel grammar for a file.
pub fn infer_language(
    file: &str,
    language: Option<Language>,
) -> Language {
    from_oxc_language(resolve_grammar(file, language))
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

/// Resolve the telarel source type for a file: the resolved grammar's module
/// kind, overlaid by an explicit option.
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_from_oxc_language_narrows_grammar() {
        assert_eq!(from_oxc_language(OxcSourceType::ts()), Language::TS);
        assert_eq!(from_oxc_language(OxcSourceType::d_ts()), Language::DTS);
        assert_eq!(from_oxc_language(OxcSourceType::jsx()), Language::JSX);
        assert_eq!(from_oxc_language(OxcSourceType::tsx()), Language::TSX);
        assert_eq!(from_oxc_language(OxcSourceType::default()), Language::JS);
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
}
