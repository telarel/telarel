use crate::_types::grammar::language::Language;
use crate::_types::grammar::source_type::SourceType;
use crate::_types::options::resolvable::ResolvableOptions;
use crate::_types::options::source_map::SourceMapOptions;

/// User options for a compile run; every field is optional and resolved
/// against defaults before use.
#[derive(Debug, Clone, Default)]
pub struct CompileOptions {
    /// Current working directory;
    /// `None` resolves to the process working directory.
    pub cwd: Option<String>,
    /// The file to be compiled;
    /// `None` resolves to a default filename.
    pub file: Option<String>,
    /// The grammar of the code;
    /// `None` infers the grammar from the file extension.
    pub language: Option<Language>,
    /// The module system of the code;
    /// `None` keeps the module kind resolved from the grammar.
    pub source_type: Option<SourceType>,
    /// The source-map inputs and outputs requested for the run.
    pub source_map: Option<SourceMapOptions>,
}

impl From<CompileOptions> for ResolvableOptions {
    fn from(options: CompileOptions) -> Self {
        Self {
            cwd: options.cwd,
            file: options.file,
            language: options.language,
            source_type: options.source_type,
            source_map: options.source_map,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constructs_compile_options() {
        let options: CompileOptions = CompileOptions {
            cwd: Some("/repo".into()),
            file: Some("src/app.ts".into()),
            language: Some(Language::TS),
            source_type: Some(SourceType::Module),
            source_map: Some(SourceMapOptions::default()),
        };

        assert_eq!(options.cwd.as_deref(), Some("/repo"));
        assert_eq!(options.file.as_deref(), Some("src/app.ts"));
        assert_eq!(options.language, Some(Language::TS));
        assert_eq!(options.source_type, Some(SourceType::Module));
        assert!(options.source_map.is_some());
    }

    #[test]
    fn test_default_compile_options_are_all_off() {
        let options: CompileOptions = CompileOptions::default();

        assert!(options.cwd.is_none());
        assert!(options.file.is_none());
        assert!(options.language.is_none());
        assert!(options.source_type.is_none());
        assert!(options.source_map.is_none());
    }

    #[test]
    fn test_from_compile_options_moves_every_field() {
        let options: CompileOptions = CompileOptions {
            cwd: Some(String::from("/repo")),
            file: Some(String::from("src/app.tsx")),
            language: Some(Language::TSX),
            source_type: Some(SourceType::Script),
            source_map: Some(SourceMapOptions::default()),
        };

        let resolvable: ResolvableOptions = ResolvableOptions::from(options);

        assert_eq!(resolvable.cwd.as_deref(), Some("/repo"));
        assert_eq!(resolvable.file.as_deref(), Some("src/app.tsx"));
        assert_eq!(resolvable.language, Some(Language::TSX));
        assert_eq!(resolvable.source_type, Some(SourceType::Script));
        assert!(resolvable.source_map.is_some());
    }
}
