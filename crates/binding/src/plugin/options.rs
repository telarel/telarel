use napi::{Error, Result, Status};

use telarel_common::{Language, SourceType};

/// Deserialize a JS-facing language string into the Rust enum.
///
/// An empty string maps to `None` (the JS hook did not set the field).
pub fn parse_language(text: &str) -> Result<Option<Language>> {
    match text {
        | "js" => Ok(Some(Language::JS)),
        | "ts" => Ok(Some(Language::TS)),
        | "dts" => Ok(Some(Language::DTS)),
        | "jsx" => Ok(Some(Language::JSX)),
        | "tsx" => Ok(Some(Language::TSX)),
        | "" => Ok(None),
        | _ => Err(Error::new(
            Status::InvalidArg,
            format!(
                "invalid language {text:?} in the `options` hook output: expected \"js\", \"ts\", \"dts\", \"jsx\" or \"tsx\""
            ),
        )),
    }
}

/// Deserialize a JS-facing source type string into the Rust enum.
///
/// An empty string maps to `None` (the JS hook did not set it).
pub fn parse_source_type(text: &str) -> Result<Option<SourceType>> {
    match text {
        | "script" => Ok(Some(SourceType::Script)),
        | "commonjs" => Ok(Some(SourceType::CommonJS)),
        | "module" => Ok(Some(SourceType::Module)),
        | "unambiguous" => Ok(Some(SourceType::Unambiguous)),
        | "" => Ok(None),
        | _ => Err(Error::new(
            Status::InvalidArg,
            format!(
                "invalid source type {text:?} in the `options` hook output: expected \"script\", \"commonjs\", \"module\" or \"unambiguous\""
            ),
        )),
    }
}

/// The JS-facing label for a grammar, `""` when unspecified.
pub fn language_label(language: Option<Language>) -> &'static str {
    match language {
        | Some(Language::JS) => "js",
        | Some(Language::TS) => "ts",
        | Some(Language::DTS) => "dts",
        | Some(Language::JSX) => "jsx",
        | Some(Language::TSX) => "tsx",
        | None => "",
    }
}

/// The JS-facing label for a module system, `""` when unspecified.
pub fn source_type_label(source_type: Option<SourceType>) -> &'static str {
    match source_type {
        | Some(SourceType::Script) => "script",
        | Some(SourceType::CommonJS) => "commonjs",
        | Some(SourceType::Module) => "module",
        | Some(SourceType::Unambiguous) => "unambiguous",
        | None => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_label_roundtrip() {
        for (text, language) in [
            ("js", Language::JS),
            ("ts", Language::TS),
            ("dts", Language::DTS),
            ("jsx", Language::JSX),
            ("tsx", Language::TSX),
        ] {
            assert_eq!(language_label(Some(language)), text, "{text}");
            assert_eq!(parse_language(text).unwrap(), Some(language), "{text}");
        }
    }

    #[test]
    fn test_source_type_label_roundtrip() {
        for (text, source_type) in [
            ("script", SourceType::Script),
            ("commonjs", SourceType::CommonJS),
            ("module", SourceType::Module),
            ("unambiguous", SourceType::Unambiguous),
        ] {
            assert_eq!(source_type_label(Some(source_type)), text, "{text}");
            assert_eq!(
                parse_source_type(text).unwrap(),
                Some(source_type),
                "{text}"
            );
        }
    }

    #[test]
    fn test_labels_empty_when_unspecified() {
        assert_eq!(language_label(None), "");
        assert_eq!(source_type_label(None), "");

        assert_eq!(parse_language("").unwrap(), None);
        assert_eq!(parse_source_type("").unwrap(), None);
    }

    #[test]
    fn test_parse_language_invalid_errors() {
        let error: napi::Error =
            parse_language("json").expect_err("invalid language errors");

        assert_eq!(error.status, napi::Status::InvalidArg);
    }

    #[test]
    fn test_parse_source_type_invalid_errors() {
        let error: napi::Error =
            parse_source_type("json").expect_err("invalid source type errors");

        assert_eq!(error.status, napi::Status::InvalidArg);
    }
}
