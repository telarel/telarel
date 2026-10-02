use telarel_common::Language;

use crate::resolve::grammar::{from_oxc_language, resolve_grammar};

/// Infer the telarel grammar for a file, mirroring `parse`'s resolution:
/// an explicit language wins; otherwise infer from the module id.
pub fn infer_language(
    file: &str,
    language: Option<Language>,
) -> Language {
    let oxc: oxc::span::SourceType = resolve_grammar(file, language);
    from_oxc_language(oxc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_infer_language_defaults_to_typescript_for_ts_file() {
        let language: Language = infer_language("index.ts", None);

        assert_eq!(language, Language::TS);
    }

    #[test]
    fn test_infer_language_defaults_to_js_for_js_file() {
        let language: Language = infer_language("index.js", None);

        assert_eq!(language, Language::JS);
    }

    #[test]
    fn test_infer_language_explicit_wins() {
        let language: Language = infer_language("index.js", Some(Language::TS));

        assert_eq!(language, Language::TS);
    }

    #[test]
    fn test_infer_language_tsx_detected() {
        let language: Language = infer_language("index.tsx", None);

        assert_eq!(language, Language::TSX);
    }

    #[test]
    fn test_infer_language_dts_detected() {
        let language: Language = infer_language("index.d.ts", None);

        assert_eq!(language, Language::DTS);
    }

    #[test]
    fn test_infer_language_jsx_detected() {
        let language: Language = infer_language("index.jsx", None);

        assert_eq!(language, Language::JSX);
    }
}
