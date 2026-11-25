use std::path::PathBuf;

use crate::_types::grammar::language::Language;
use crate::_types::grammar::source_type::SourceType;
use crate::_types::options::builtin::generator::GeneratorOptions;
use crate::_types::options::builtin::parser::ParserOptions;
use crate::_types::options::output::OutputOptions;
use crate::_types::options::sourcemap::SourceMapOptions;

/// User options for a compile run.
///
/// Every field is optional. Each field resolves against defaults before use.
#[derive(Debug, Clone, Default)]
pub struct CompileOptions {
    /// Current working directory.
    ///
    /// By default, it is the process working directory.
    pub cwd: Option<PathBuf>,
    /// The file being compiled.
    ///
    /// By default, it is `"index.ts"`.
    pub file: Option<String>,
    /// The grammar of the code.
    ///
    /// By default, it is inferred from the file extension.
    pub language: Option<Language>,
    /// The module system of the code.
    ///
    /// By default, it is resolved from the grammar.
    pub source_type: Option<SourceType>,
    /// Parser-stage options.
    ///
    /// By default, it is `None`, which applies the `ParserOptions` defaults.
    pub parser: Option<ParserOptions>,
    /// Generator-stage options.
    ///
    /// By default, it is `None`, which applies the `GeneratorOptions` defaults.
    pub generator: Option<GeneratorOptions>,
    /// The output code configuration.
    ///
    /// By default, it is `None`.
    pub output: Option<OutputOptions>,
    /// The source-map inputs and outputs requested for the run.
    ///
    /// By default, it is `None`, which disables source-map input lookup and
    /// output delivery.
    pub source_map: Option<SourceMapOptions>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constructs_compile_options() {
        let options: CompileOptions = CompileOptions {
            cwd: Some(PathBuf::from("/repo")),
            file: Some("src/app.ts".into()),
            language: Some(Language::TS),
            source_type: Some(SourceType::Module),
            parser: Some(ParserOptions {
                preserve_parens: Some(true),
                ..ParserOptions::default()
            }),
            generator: Some(GeneratorOptions {
                single_quote: Some(true),
                ..GeneratorOptions::default()
            }),
            output: Some(OutputOptions {
                file: Some(String::from("dist/app.js")),
            }),
            source_map: Some(SourceMapOptions::default()),
        };

        assert_eq!(options.cwd, Some(PathBuf::from("/repo")));
        assert_eq!(options.file.as_deref(), Some("src/app.ts"));
        assert_eq!(options.language, Some(Language::TS));
        assert_eq!(options.source_type, Some(SourceType::Module));
        assert_eq!(
            options.output.as_ref().and_then(|output| output.file.as_deref()),
            Some("dist/app.js"),
        );
        assert!(options.source_map.is_some());
        assert_eq!(
            options.parser.as_ref().and_then(|parser| parser.preserve_parens),
            Some(true),
        );
        assert_eq!(
            options
                .generator
                .as_ref()
                .and_then(|generator| generator.single_quote),
            Some(true),
        );
    }

    #[test]
    fn test_default_compile_options_are_all_off() {
        let options: CompileOptions = CompileOptions::default();

        assert!(options.cwd.is_none());
        assert!(options.file.is_none());
        assert!(options.language.is_none());
        assert!(options.source_type.is_none());
        assert!(options.output.is_none());
        assert!(options.source_map.is_none());
        assert!(options.parser.is_none());
        assert!(options.generator.is_none());
    }
}
