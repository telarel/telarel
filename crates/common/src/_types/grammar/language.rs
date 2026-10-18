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
