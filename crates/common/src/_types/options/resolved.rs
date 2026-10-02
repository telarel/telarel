use crate::_types::options::language::Language;
use crate::_types::options::source_type::SourceType;

/// Resolved options, as seen by `compile_start` and later hooks.
#[derive(Debug, Clone)]
pub struct ResolvedOptions {
    /// Current working directory, resolved.
    pub cwd: String,
    /// The file to be compiled.
    pub file: String,
    /// The code to be compiled.
    pub code: String,
    /// The grammar of the code, defaulted from the file extension.
    pub language: Language,
    /// The module system of the code, resolved from the grammar.
    pub source_type: SourceType,
    /// The settled plugin-name list, after the options fixpoint.
    pub plugins: Vec<String>,
}

#[cfg(test)]
mod tests {
    use crate::_types::options::language::Language;
    use crate::_types::options::source_type::SourceType;

    use super::*;

    #[test]
    fn test_constructs_resolved_options() {
        let options: ResolvedOptions = ResolvedOptions {
            cwd: String::from("/repo"),
            file: String::from("src/app.ts"),
            code: String::from("let a = 1;"),
            language: Language::TS,
            source_type: SourceType::Module,
            plugins: vec![String::from("probe")],
        };

        assert_eq!(options.cwd, "/repo");
        assert_eq!(options.file, "src/app.ts");
        assert_eq!(options.code, "let a = 1;");
        assert_eq!(options.language, Language::TS);
        assert_eq!(options.source_type, SourceType::Module);
        assert_eq!(options.plugins, vec![String::from("probe")]);
    }
}
