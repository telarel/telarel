use std::fmt;
use std::str::FromStr;

use oxc::span::SourceType as OxcSourceType;

/// The grammar of the source code.
#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    /// JavaScript grammar (as for a `.js` file).
    JS,
    /// TypeScript grammar (as for a `.ts` file).
    TS,
    /// TypeScript declaration grammar (as for a `.d.ts` file).
    DTS,
    /// JavaScript grammar with JSX (as for a `.jsx` file).
    JSX,
    /// TypeScript grammar with JSX (as for a `.tsx` file).
    TSX,
}

impl Language {
    /// The canonical lowercase label used on the JavaScript boundary.
    pub const fn as_str(self) -> &'static str {
        match self {
            | Language::JS => "js",
            | Language::TS => "ts",
            | Language::DTS => "dts",
            | Language::JSX => "jsx",
            | Language::TSX => "tsx",
        }
    }
}

impl fmt::Display for Language {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Language {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text {
            | "js" => Ok(Language::JS),
            | "ts" => Ok(Language::TS),
            | "dts" => Ok(Language::DTS),
            | "jsx" => Ok(Language::JSX),
            | "tsx" => Ok(Language::TSX),
            | _ => Err(format!(
                "invalid language {text:?}: expected \"js\", \"ts\", \"dts\", \"jsx\" or \"tsx\""
            )),
        }
    }
}

/// Resolve the oxc source type for an explicit grammar.
pub fn grammar_source_type(language: Language) -> OxcSourceType {
    let source_type: OxcSourceType = match language {
        | Language::JS => OxcSourceType::unambiguous(),
        | Language::TS => OxcSourceType::unambiguous().with_typescript(true),
        | Language::DTS => OxcSourceType::d_ts(),
        | Language::JSX => OxcSourceType::unambiguous().with_jsx(true),
        | Language::TSX => {
            OxcSourceType::unambiguous().with_typescript(true).with_jsx(true)
        },
    };

    source_type
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_labels_roundtrip() {
        let expected: [(&str, Language); 5] = [
            ("js", Language::JS),
            ("ts", Language::TS),
            ("dts", Language::DTS),
            ("jsx", Language::JSX),
            ("tsx", Language::TSX),
        ];

        for (label, language) in expected {
            assert_eq!(language.as_str(), label, "{label}");
            assert_eq!(language.to_string(), label, "{label}");
            assert_eq!(label.parse::<Language>(), Ok(language), "{label}");
        }
    }

    #[test]
    fn test_from_str_rejects_non_canonical_labels() {
        for text in ["", "TS", "Js", "d.ts", "json"] {
            let error: String =
                text.parse::<Language>().expect_err("invalid label errors");

            assert_eq!(
                error,
                format!(
                    "invalid language {text:?}: expected \"js\", \"ts\", \"dts\", \"jsx\" or \"tsx\""
                ),
                "{text}"
            );
        }
    }
}
