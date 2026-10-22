use std::path::PathBuf;

use crate::_types::grammar::language::Language;
use crate::_types::grammar::source_type::SourceType;
use crate::_types::options::output::ResolvedOutputOptions;
use crate::_types::options::sourcemap::SourceMapOptions;

/// Resolved options, as seen by `start` and later hooks.
#[derive(Debug, Clone)]
pub struct ResolvedOptions {
    /// Working directory used to resolve relative paths.
    pub cwd: PathBuf,
    /// The file being compiled.
    pub file: String,
    /// The grammar of the code, defaulted from the file extension.
    pub language: Language,
    /// The module system of the code, resolved from the grammar.
    pub source_type: SourceType,
    /// The output code settings.
    pub output: ResolvedOutputOptions,
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
            cwd: PathBuf::from("/repo"),
            file: String::from("src/app.ts"),
            language: Language::TS,
            source_type: SourceType::Module,
            output: ResolvedOutputOptions {
                file: String::from("dist/src/app.js"),
            },
            source_map: SourceMapOptions::default(),
            plugins: vec![String::from("probe")],
        };

        assert_eq!(options.cwd, PathBuf::from("/repo"));
        assert_eq!(options.file, "src/app.ts");
        assert_eq!(options.language, Language::TS);
        assert_eq!(options.source_type, SourceType::Module);
        assert_eq!(options.output.file, "dist/src/app.js");
        assert!(options.source_map.input.is_none());
        assert!(options.source_map.output.is_none());
        assert_eq!(options.plugins, vec![String::from("probe")]);
    }
}
