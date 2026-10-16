use crate::_types::grammar::language::Language;
use crate::_types::grammar::source_type::SourceType;
use crate::_types::options::source_map::SourceMapOptions;

/// Resolved options, as seen by `start` and later hooks.
#[derive(Debug, Clone)]
pub struct ResolvedOptions {
    /// Current working directory, resolved.
    pub cwd: String,
    /// The file to be compiled.
    pub file: String,
    /// The grammar of the code, defaulted from the file extension.
    pub language: Language,
    /// The module system of the code, resolved from the grammar.
    pub source_type: SourceType,
    /// The source-map inputs and outputs requested for the run.
    pub source_map: SourceMapOptions,
    /// The settled plugin-name list, after the options fixpoint.
    pub plugins: Vec<String>,
}

#[cfg(test)]
mod tests {
    use crate::_types::grammar::language::Language;
    use crate::_types::grammar::source_type::SourceType;

    use super::*;

    #[test]
    fn test_constructs_resolved_options() {
        let options: ResolvedOptions = ResolvedOptions {
            cwd: String::from("/repo"),
            file: String::from("src/app.ts"),
            language: Language::TS,
            source_type: SourceType::Module,
            source_map: SourceMapOptions::default(),
            plugins: vec![String::from("probe")],
        };

        assert_eq!(options.cwd, "/repo");
        assert_eq!(options.file, "src/app.ts");
        assert_eq!(options.language, Language::TS);
        assert_eq!(options.source_type, SourceType::Module);
        assert!(options.source_map.input.is_none());
        assert!(options.source_map.output.is_none());
        assert_eq!(options.plugins, vec![String::from("probe")]);
    }
}
