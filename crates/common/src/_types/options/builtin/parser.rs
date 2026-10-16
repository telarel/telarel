use crate::_types::grammar::language::Language;
use crate::_types::grammar::source_type::SourceType;
use crate::_types::options::resolved::ResolvedOptions;
use crate::_types::options::source_map::SourceMapOptions;

/// Resolved options for the builtin [`crate::ast::parser::Parser`].
#[derive(Debug, Clone)]
pub struct ParserOptions {
    /// Current working directory.
    pub cwd: String,
    /// The file being parsed.
    pub file: String,
    /// The grammar of the code.
    pub language: Language,
    /// The module system of the code.
    pub source_type: SourceType,
    /// The source-map inputs and outputs requested for the step.
    pub source_map: SourceMapOptions,
}

impl From<&ResolvedOptions> for ParserOptions {
    fn from(options: &ResolvedOptions) -> Self {
        Self {
            cwd: options.cwd.clone(),
            file: options.file.clone(),
            language: options.language,
            source_type: options.source_type,
            source_map: options.source_map.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::_types::grammar::language::Language;
    use crate::_types::grammar::source_type::SourceType;
    use crate::_types::options::source_map::SourceMapOptions;

    use super::*;

    fn resolved_options() -> ResolvedOptions {
        ResolvedOptions {
            cwd: String::from("/repo"),
            file: String::from("src/app.ts"),
            language: Language::TS,
            source_type: SourceType::Module,
            source_map: SourceMapOptions::default(),
            plugins: vec![String::from("probe")],
        }
    }

    #[test]
    fn test_from_resolved_options_maps_every_field() {
        let resolved: ResolvedOptions = resolved_options();

        let options: ParserOptions = ParserOptions::from(&resolved);

        assert_eq!(options.cwd, "/repo");
        assert_eq!(options.file, "src/app.ts");
        assert_eq!(options.language, Language::TS);
        assert_eq!(options.source_type, SourceType::Module);
        assert!(options.source_map.input.is_none());
        assert!(options.source_map.output.is_none());
    }
}
