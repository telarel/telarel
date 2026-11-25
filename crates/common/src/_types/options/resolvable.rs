use std::path::PathBuf;

use crate::_types::grammar::language::Language;
use crate::_types::grammar::source_type::SourceType;
use crate::_types::options::builtin::generator::GeneratorOptions;
use crate::_types::options::builtin::parser::ParserOptions;
use crate::_types::options::functions::compile::CompileOptions;
use crate::_types::options::functions::compile_from_ast::CompileFromAstOptions;
use crate::_types::options::functions::generate::GenerateOptions;
use crate::_types::options::functions::parse::ParseOptions;
use crate::_types::options::functions::rewrite::RewriteOptions;
use crate::_types::options::output::OutputOptions;
use crate::_types::options::sourcemap::SourceMapOptions;
use crate::_types::options::sourcemap::input::InputSourceMap;

/// The union of every step's options, resolved once per compile run.
///
/// Every field is optional.
#[derive(Debug, Clone, Default)]
pub struct ResolvableOptions {
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
    /// By default, it is taken from the input AST grammar, then inferred
    /// from the file extension.
    pub language: Option<Language>,
    /// The module kind of the code.
    ///
    /// By default, it is taken from the input AST module kind, then resolved
    /// from the file extension.
    pub source_type: Option<SourceType>,
    /// Parser-stage options requested for the run.
    ///
    /// By default, it is `None`, which applies the `ParserOptions` defaults.
    pub parser: Option<ParserOptions>,
    /// Generator-stage options requested for the run.
    ///
    /// By default, it is `None`, which applies the `GeneratorOptions`
    /// defaults.
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

fn wrap_input_source_map(
    input: Option<InputSourceMap>
) -> Option<SourceMapOptions> {
    input.map(|input: InputSourceMap| SourceMapOptions {
        input: Some(input),
        output: None,
    })
}

impl From<CompileOptions> for ResolvableOptions {
    fn from(options: CompileOptions) -> Self {
        Self {
            cwd: options.cwd,
            file: options.file,
            language: options.language,
            source_type: options.source_type,
            parser: options.parser,
            generator: options.generator,
            output: options.output,
            source_map: options.source_map,
        }
    }
}

impl From<ParseOptions> for ResolvableOptions {
    fn from(options: ParseOptions) -> Self {
        Self {
            cwd: options.cwd,
            file: options.file,
            language: options.language,
            source_type: options.source_type,
            parser: options.parser,
            generator: None,
            output: None,
            source_map: wrap_input_source_map(options.input_source_map),
        }
    }
}

impl From<RewriteOptions> for ResolvableOptions {
    fn from(options: RewriteOptions) -> Self {
        Self {
            cwd: options.cwd,
            file: options.file,
            language: options.language,
            source_type: options.source_type,
            parser: None,
            generator: None,
            output: None,
            source_map: wrap_input_source_map(options.input_source_map),
        }
    }
}

impl From<GenerateOptions> for ResolvableOptions {
    fn from(options: GenerateOptions) -> Self {
        Self {
            cwd: options.cwd,
            file: options.file,
            language: options.language,
            source_type: None,
            parser: None,
            generator: options.generator,
            output: options.output,
            source_map: options.source_map,
        }
    }
}

impl From<CompileFromAstOptions> for ResolvableOptions {
    fn from(options: CompileFromAstOptions) -> Self {
        Self {
            cwd: options.cwd,
            file: options.file,
            language: options.language,
            source_type: options.source_type,
            parser: None,
            generator: options.generator,
            output: options.output,
            source_map: options.source_map,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::_types::options::sourcemap::input::InputSourceMapMode;

    use super::*;

    #[test]
    fn test_from_compile_options_moves_every_field() {
        let options: CompileOptions = CompileOptions {
            cwd: Some(PathBuf::from("/repo")),
            file: Some(String::from("src/app.tsx")),
            language: Some(Language::TSX),
            source_type: Some(SourceType::Script),
            parser: Some(ParserOptions {
                allow_return_outside_function: Some(true),
                ..ParserOptions::default()
            }),
            generator: Some(GeneratorOptions {
                minify: Some(true),
                ..GeneratorOptions::default()
            }),
            output: Some(OutputOptions {
                file: Some(String::from("dist/app.mjs")),
            }),
            source_map: Some(SourceMapOptions::default()),
        };

        let resolvable: ResolvableOptions = ResolvableOptions::from(options);

        assert_eq!(resolvable.cwd, Some(PathBuf::from("/repo")));
        assert_eq!(resolvable.file.as_deref(), Some("src/app.tsx"));
        assert_eq!(resolvable.language, Some(Language::TSX));
        assert_eq!(resolvable.source_type, Some(SourceType::Script));
        assert_eq!(
            resolvable
                .output
                .as_ref()
                .and_then(|output| output.file.as_deref()),
            Some("dist/app.mjs"),
        );
        assert!(resolvable.source_map.is_some());
        assert_eq!(
            resolvable
                .parser
                .as_ref()
                .and_then(|parser| parser.allow_return_outside_function),
            Some(true),
        );
        assert_eq!(
            resolvable
                .generator
                .as_ref()
                .and_then(|generator| generator.minify),
            Some(true),
        );
    }

    #[test]
    fn test_from_parse_options_wraps_input_source_map() {
        let options: ParseOptions = ParseOptions {
            cwd: Some(PathBuf::from("/repo")),
            file: Some(String::from("src/app.ts")),
            language: Some(Language::TS),
            source_type: Some(SourceType::Module),
            parser: Some(ParserOptions {
                preserve_parens: Some(true),
                ..ParserOptions::default()
            }),
            input_source_map: Some(InputSourceMap::Mode(
                InputSourceMapMode::Auto,
            )),
        };

        let resolvable: ResolvableOptions = ResolvableOptions::from(options);

        assert_eq!(resolvable.cwd, Some(PathBuf::from("/repo")));
        assert_eq!(resolvable.file.as_deref(), Some("src/app.ts"));
        assert_eq!(resolvable.language, Some(Language::TS));
        assert_eq!(resolvable.source_type, Some(SourceType::Module));
        assert_eq!(
            resolvable
                .parser
                .as_ref()
                .and_then(|parser| parser.preserve_parens),
            Some(true),
        );
        assert!(resolvable.generator.is_none());

        let source_map: SourceMapOptions =
            resolvable.source_map.expect("input map is carried");

        assert!(matches!(
            source_map.input,
            Some(InputSourceMap::Mode(InputSourceMapMode::Auto)),
        ));
        assert!(source_map.output.is_none());
    }

    #[test]
    fn test_from_rewrite_options_wraps_input_source_map() {
        let options: RewriteOptions = RewriteOptions {
            cwd: Some(PathBuf::from("/repo")),
            file: Some(String::from("src/app.tsx")),
            language: Some(Language::TSX),
            source_type: Some(SourceType::Script),
            input_source_map: Some(InputSourceMap::Mode(
                InputSourceMapMode::Inline,
            )),
        };

        let resolvable: ResolvableOptions = ResolvableOptions::from(options);

        assert_eq!(resolvable.cwd, Some(PathBuf::from("/repo")));
        assert_eq!(resolvable.file.as_deref(), Some("src/app.tsx"));
        assert_eq!(resolvable.language, Some(Language::TSX));
        assert_eq!(resolvable.source_type, Some(SourceType::Script));
        assert!(resolvable.parser.is_none());
        assert!(resolvable.generator.is_none());

        let source_map: SourceMapOptions =
            resolvable.source_map.expect("input map is carried");

        assert!(matches!(
            source_map.input,
            Some(InputSourceMap::Mode(InputSourceMapMode::Inline)),
        ));
        assert!(source_map.output.is_none());
    }

    #[test]
    fn test_from_generate_options_preserves_language() {
        let options: GenerateOptions = GenerateOptions {
            language: Some(Language::TSX),
            ..Default::default()
        };

        let resolvable: ResolvableOptions = ResolvableOptions::from(options);

        assert_eq!(resolvable.language, Some(Language::TSX));
    }

    #[test]
    fn test_from_generate_options_carries_language_and_drops_source_type() {
        let options: GenerateOptions = GenerateOptions {
            cwd: Some(PathBuf::from("/repo")),
            file: Some(String::from("src/app.js")),
            language: Some(Language::TSX),
            generator: Some(GeneratorOptions {
                single_quote: Some(true),
                ..GeneratorOptions::default()
            }),
            output: Some(OutputOptions {
                file: Some(String::from("dist/app.js")),
            }),
            source_map: Some(SourceMapOptions::default()),
        };

        let resolvable: ResolvableOptions = ResolvableOptions::from(options);

        assert_eq!(resolvable.cwd, Some(PathBuf::from("/repo")));
        assert_eq!(resolvable.file.as_deref(), Some("src/app.js"));
        assert_eq!(resolvable.language, Some(Language::TSX));
        assert_eq!(resolvable.source_type, None);
        assert!(resolvable.parser.is_none());
        assert_eq!(
            resolvable
                .generator
                .as_ref()
                .and_then(|generator| generator.single_quote),
            Some(true),
        );
        assert_eq!(
            resolvable
                .output
                .as_ref()
                .and_then(|output| output.file.as_deref()),
            Some("dist/app.js"),
        );
        assert!(resolvable.source_map.is_some());
    }

    #[test]
    fn test_from_compile_from_ast_options_moves_every_field() {
        let options: CompileFromAstOptions = CompileFromAstOptions {
            cwd: Some(PathBuf::from("/repo")),
            file: Some(String::from("src/app.mjs")),
            language: Some(Language::JS),
            source_type: Some(SourceType::CommonJS),
            generator: Some(GeneratorOptions {
                ascii_only: Some(true),
                ..GeneratorOptions::default()
            }),
            output: Some(OutputOptions {
                file: Some(String::from("dist/app.mjs")),
            }),
            source_map: Some(SourceMapOptions::default()),
        };

        let resolvable: ResolvableOptions = ResolvableOptions::from(options);

        assert_eq!(resolvable.cwd, Some(PathBuf::from("/repo")));
        assert_eq!(resolvable.file.as_deref(), Some("src/app.mjs"));
        assert_eq!(resolvable.language, Some(Language::JS));
        assert_eq!(resolvable.source_type, Some(SourceType::CommonJS));
        assert!(resolvable.parser.is_none());
        assert_eq!(
            resolvable
                .generator
                .as_ref()
                .and_then(|generator| generator.ascii_only),
            Some(true),
        );
        assert_eq!(
            resolvable
                .output
                .as_ref()
                .and_then(|output| output.file.as_deref()),
            Some("dist/app.mjs"),
        );
        assert!(resolvable.source_map.is_some());
    }
}
