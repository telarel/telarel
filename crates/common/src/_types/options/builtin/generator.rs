use crate::_types::options::resolved::ResolvedOptions;
use crate::_types::options::source_map::SourceMapOptions;

/// Resolved options for the builtin [`crate::ast::generator::Generator`].
#[derive(Debug, Clone)]
pub struct GeneratorOptions {
    /// Current working directory (reserved; builtin codegen does not use it).
    pub cwd: String,
    /// Path of the file being generated.
    pub file: String,
    /// The source-map inputs and outputs requested for the step.
    pub source_map: SourceMapOptions,
}

impl From<&ResolvedOptions> for GeneratorOptions {
    fn from(options: &ResolvedOptions) -> Self {
        Self {
            cwd: options.cwd.clone(),
            file: options.file.clone(),
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
            file: String::from("src/app.js"),
            language: Language::JS,
            source_type: SourceType::Module,
            source_map: SourceMapOptions::default(),
            plugins: vec![String::from("probe")],
        }
    }

    #[test]
    fn test_from_resolved_options_maps_cwd_file_and_source_map() {
        let resolved: ResolvedOptions = resolved_options();

        let options: GeneratorOptions = GeneratorOptions::from(&resolved);

        assert_eq!(options.cwd, "/repo");
        assert_eq!(options.file, "src/app.js");
        assert!(options.source_map.input.is_none());
        assert!(options.source_map.output.is_none());
    }
}
