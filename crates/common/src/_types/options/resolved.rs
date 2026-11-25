use std::path::PathBuf;

use crate::_types::grammar::language::Language;
use crate::_types::grammar::source_type::SourceType;
use crate::_types::options::builtin::generator::ResolvedGeneratorOptions;
use crate::_types::options::builtin::generator::ResolvedGeneratorStageOptions;
use crate::_types::options::builtin::parser::ResolvedParserOptions;
use crate::_types::options::builtin::parser::ResolvedParserStageOptions;
use crate::_types::options::output::ResolvedOutputOptions;
use crate::_types::options::sourcemap::SourceMapOptions;

/// Resolved options, as seen by `start` and later hooks.
#[derive(Debug, Clone)]
pub struct ResolvedOptions {
    /// Working directory used to resolve relative paths.
    pub cwd: PathBuf,
    /// The file being compiled.
    pub file: String,
    /// The grammar of the code, resolved from options, the AST, or the file
    /// extension.
    pub language: Language,
    /// The module kind of the code, resolved from options, the AST, or the
    /// file extension.
    pub source_type: SourceType,
    /// The merged parser-stage settings the builtin parser and `parse` hooks
    /// receive.
    pub parser: ResolvedParserStageOptions,
    /// The merged generator-stage settings the builtin generator and
    /// `generate` hooks receive.
    pub generator: ResolvedGeneratorStageOptions,
    /// The output code configuration.
    pub output: ResolvedOutputOptions,
    /// The source-map inputs and outputs requested for the run.
    pub source_map: SourceMapOptions,
    /// The settled plugin-name list, after the options fixpoint.
    pub plugins: Vec<String>,
}

impl From<&ResolvedOptions> for ResolvedParserOptions {
    fn from(options: &ResolvedOptions) -> Self {
        Self {
            cwd: options.cwd.clone(),
            file: options.file.clone(),
            language: options.language,
            source_type: options.source_type,
            source_map: options.source_map.clone(),
            preserve_parens: options.parser.preserve_parens,
            allow_return_outside_function: options
                .parser
                .allow_return_outside_function,
            oxc: options.parser.oxc,
        }
    }
}

impl From<&ResolvedOptions> for ResolvedGeneratorOptions {
    fn from(options: &ResolvedOptions) -> Self {
        Self {
            cwd: options.cwd.clone(),
            file: options.file.clone(),
            source_map: options.source_map.clone(),
            single_quote: options.generator.single_quote,
            ascii_only: options.generator.ascii_only,
            indent_style: options.generator.indent_style,
            indent_width: options.generator.indent_width,
            comments: options.generator.comments,
            minify: options.generator.minify,
            oxc: options.generator.oxc.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use oxc::codegen::{CommentOptions, IndentChar};

    use crate::_types::grammar::language::Language;
    use crate::_types::grammar::source_type::SourceType;
    use crate::_types::options::builtin::generator::IndentStyle;
    use crate::_types::options::sourcemap::input::{
        InputSourceMap, InputSourceMapMode,
    };
    use crate::_types::options::sourcemap::output::{
        OutputSourceMap, OutputSourceMapMode,
    };
    use crate::_types::options::{OxcCodegenOptions, OxcParseOptions};

    use super::*;

    fn resolved_options_for_parser() -> ResolvedOptions {
        ResolvedOptions {
            cwd: PathBuf::from("/repo"),
            file: String::from("src/app.ts"),
            language: Language::TS,
            source_type: SourceType::Module,
            parser: ResolvedParserStageOptions {
                preserve_parens: true,
                allow_return_outside_function: true,
                oxc: OxcParseOptions {
                    preserve_parens: true,
                    allow_return_outside_function: true,
                    enable_ident_hashes: false,
                    ..OxcParseOptions::default()
                },
            },
            generator: ResolvedGeneratorStageOptions::default(),
            output: ResolvedOutputOptions {
                file: String::from("dist/src/app.js"),
            },
            source_map: SourceMapOptions {
                input: Some(InputSourceMap::Mode(InputSourceMapMode::Auto)),
                output: Some(OutputSourceMap::Mode(OutputSourceMapMode::Both)),
            },
            plugins: vec![String::from("probe")],
        }
    }

    fn resolved_options_for_generator() -> ResolvedOptions {
        ResolvedOptions {
            cwd: PathBuf::from("/repo"),
            file: String::from("src/app.js"),
            language: Language::JS,
            source_type: SourceType::Module,
            parser: ResolvedParserStageOptions::default(),
            generator: ResolvedGeneratorStageOptions {
                single_quote: true,
                ascii_only: true,
                indent_style: IndentStyle::Tab,
                indent_width: 4,
                comments: false,
                minify: true,
                oxc: OxcCodegenOptions {
                    single_quote: true,
                    ascii_only: true,
                    indent_char: IndentChar::Tab,
                    indent_width: 4,
                    comments: CommentOptions::disabled(),
                    minify: true,
                    ..OxcCodegenOptions::default()
                },
            },
            output: ResolvedOutputOptions {
                file: String::from("dist/src/app.js"),
            },
            source_map: SourceMapOptions {
                input: Some(InputSourceMap::Mode(InputSourceMapMode::Auto)),
                output: Some(OutputSourceMap::Mode(OutputSourceMapMode::Both)),
            },
            plugins: vec![String::from("probe")],
        }
    }

    #[test]
    fn test_constructs_resolved_options() {
        let options: ResolvedOptions = ResolvedOptions {
            cwd: PathBuf::from("/repo"),
            file: String::from("src/app.ts"),
            language: Language::TS,
            source_type: SourceType::Module,
            parser: ResolvedParserStageOptions::default(),
            generator: ResolvedGeneratorStageOptions::default(),
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
        assert!(!options.parser.preserve_parens);
        assert!(!options.parser.allow_return_outside_function);
        assert_eq!(options.generator.indent_style, IndentStyle::Space);
        assert_eq!(options.generator.indent_width, 2);
        assert!(options.generator.comments);
        assert!(!options.generator.minify);
        assert_eq!(options.output.file, "dist/src/app.js");
        assert!(options.source_map.input.is_none());
        assert!(options.source_map.output.is_none());
        assert_eq!(options.plugins, vec![String::from("probe")]);
    }

    #[test]
    fn test_from_resolved_options_maps_every_parser_field() {
        let resolved: ResolvedOptions = resolved_options_for_parser();

        let options: ResolvedParserOptions =
            ResolvedParserOptions::from(&resolved);

        assert_eq!(options.cwd, PathBuf::from("/repo"));
        assert_eq!(options.file, "src/app.ts");
        assert_eq!(options.language, Language::TS);
        assert_eq!(options.source_type, SourceType::Module);
        assert!(matches!(
            options.source_map.input,
            Some(InputSourceMap::Mode(InputSourceMapMode::Auto)),
        ));
        assert!(matches!(
            options.source_map.output,
            Some(OutputSourceMap::Mode(OutputSourceMapMode::Both)),
        ));
        assert!(options.preserve_parens);
        assert!(options.allow_return_outside_function);
        assert!(options.oxc.preserve_parens);
        assert!(options.oxc.allow_return_outside_function);
        assert!(!options.oxc.enable_ident_hashes);
    }

    #[test]
    fn test_from_resolved_options_maps_every_generator_field() {
        let resolved: ResolvedOptions = resolved_options_for_generator();

        let options: ResolvedGeneratorOptions =
            ResolvedGeneratorOptions::from(&resolved);

        assert_eq!(options.cwd, PathBuf::from("/repo"));
        assert_eq!(options.file, "src/app.js");
        assert!(matches!(
            options.source_map.input,
            Some(InputSourceMap::Mode(InputSourceMapMode::Auto)),
        ));
        assert!(matches!(
            options.source_map.output,
            Some(OutputSourceMap::Mode(OutputSourceMapMode::Both)),
        ));
        assert!(options.single_quote);
        assert!(options.ascii_only);
        assert_eq!(options.indent_style, IndentStyle::Tab);
        assert_eq!(options.indent_width, 4);
        assert!(!options.comments);
        assert!(options.minify);
        assert!(options.oxc.single_quote);
        assert!(options.oxc.ascii_only);
        assert_eq!(options.oxc.indent_char, IndentChar::Tab);
        assert_eq!(options.oxc.indent_width, 4);
        assert_eq!(options.oxc.comments, CommentOptions::disabled());
        assert!(options.oxc.minify);
    }
}
