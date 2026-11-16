use std::path::PathBuf;

use crate::_types::grammar::language::Language;
use crate::_types::grammar::source_type::SourceType;
use crate::_types::options::OxcParseOptions;
use crate::_types::options::resolved::ResolvedOptions;
use crate::_types::options::sourcemap::SourceMapOptions;
use crate::helpers::resolve::resolve_parser_stage_options;

/// A partial patch of raw oxc parser options.
///
/// Only fields set to `Some` are applied. A default patch sets no fields and
/// therefore changes nothing.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PartialOxcParseOptions {
    /// Whether regular-expression literals are parsed into AST nodes.
    pub parse_regular_expression: Option<bool>,

    /// Whether `return` is accepted outside a function body.
    pub allow_return_outside_function: Option<bool>,

    /// Whether parenthesized expressions stay in the AST.
    pub preserve_parens: Option<bool>,

    /// Whether V8 intrinsic calls are accepted.
    pub allow_v8_intrinsics: Option<bool>,

    /// Whether identifier names carry precomputed hashes.
    pub enable_ident_hashes: Option<bool>,
}

impl PartialOxcParseOptions {
    /// Apply every set field to `options`. Unset fields leave `options`
    /// unchanged.
    pub fn apply_to(
        &self,
        options: &mut OxcParseOptions,
    ) {
        if let Some(value) = self.parse_regular_expression {
            options.parse_regular_expression = value;
        }

        if let Some(value) = self.allow_return_outside_function {
            options.allow_return_outside_function = value;
        }

        if let Some(value) = self.preserve_parens {
            options.preserve_parens = value;
        }

        if let Some(value) = self.allow_v8_intrinsics {
            options.allow_v8_intrinsics = value;
        }

        if let Some(value) = self.enable_ident_hashes {
            options.enable_ident_hashes = value;
        }
    }
}

/// Parser-stage options requested for a run.
#[derive(Debug, Clone, Default)]
pub struct ParserOptions {
    /// Whether parenthesized expressions stay in the AST as
    /// `ParenthesizedExpression` nodes.
    ///
    /// By default, it is `false`.
    pub preserve_parens: Option<bool>,

    /// Whether `return` is accepted outside a function body.
    ///
    /// By default, it is `false`.
    pub allow_return_outside_function: Option<bool>,

    /// A partial patch of raw oxc parser options applied over the Telarel
    /// defaults.
    ///
    /// Only set fields override the defaults. A curated field wins over the
    /// matching raw field. An unset or empty patch changes nothing.
    pub oxc: Option<PartialOxcParseOptions>,
}

/// Effective parser-stage settings.
///
/// `oxc` is the full merged object and the curated fields mirror it.
#[derive(Debug, Clone)]
pub struct ResolvedParserStageOptions {
    /// Whether parenthesized expressions stay in the AST.
    pub preserve_parens: bool,

    /// Whether `return` is accepted outside a function body.
    pub allow_return_outside_function: bool,

    /// The effective options for the raw oxc parser.
    pub oxc: OxcParseOptions,
}

impl Default for ResolvedParserStageOptions {
    fn default() -> Self {
        resolve_parser_stage_options(None)
    }
}

/// Resolved options for the builtin [`crate::ast::parser::Parser`].
#[derive(Debug, Clone)]
pub struct ResolvedParserOptions {
    /// Resolved working directory.
    pub cwd: PathBuf,
    /// The file being parsed.
    pub file: String,
    /// Source language of the file.
    pub language: Language,
    /// Module system requested for parsing.
    pub source_type: SourceType,
    /// The source-map inputs and outputs requested for the step.
    pub source_map: SourceMapOptions,
    /// Whether parenthesized expressions stay in the AST.
    pub preserve_parens: bool,
    /// Whether `return` is accepted outside a function body.
    pub allow_return_outside_function: bool,
    /// The effective options for the raw oxc parser.
    pub oxc: OxcParseOptions,
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

#[cfg(test)]
mod tests {
    use crate::_types::grammar::language::Language;
    use crate::_types::grammar::source_type::SourceType;
    use crate::_types::options::builtin::generator::ResolvedGeneratorStageOptions;
    use crate::_types::options::output::ResolvedOutputOptions;
    use crate::_types::options::sourcemap::SourceMapOptions;
    use crate::_types::options::sourcemap::input::{
        InputSourceMap, InputSourceMapMode,
    };
    use crate::_types::options::sourcemap::output::{
        OutputSourceMap, OutputSourceMapMode,
    };

    use super::*;

    fn resolved_options() -> ResolvedOptions {
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

    #[test]
    fn test_parser_options_default_is_all_unset() {
        let options: ParserOptions = ParserOptions::default();

        assert!(options.preserve_parens.is_none());
        assert!(options.allow_return_outside_function.is_none());
        assert!(options.oxc.is_none());
    }

    #[test]
    fn test_partial_oxc_parse_options_default_is_a_no_op() {
        let mut options: OxcParseOptions = OxcParseOptions::default();

        PartialOxcParseOptions::default().apply_to(&mut options);

        assert_eq!(
            options.parse_regular_expression,
            OxcParseOptions::default().parse_regular_expression
        );
        assert_eq!(
            options.allow_return_outside_function,
            OxcParseOptions::default().allow_return_outside_function
        );
        assert_eq!(
            options.preserve_parens,
            OxcParseOptions::default().preserve_parens
        );
        assert_eq!(
            options.allow_v8_intrinsics,
            OxcParseOptions::default().allow_v8_intrinsics
        );
        assert_eq!(
            options.enable_ident_hashes,
            OxcParseOptions::default().enable_ident_hashes
        );
    }

    #[test]
    fn test_partial_oxc_parse_options_applies_only_set_fields() {
        let mut options: OxcParseOptions = OxcParseOptions::default();

        PartialOxcParseOptions {
            parse_regular_expression: Some(true),
            ..PartialOxcParseOptions::default()
        }
        .apply_to(&mut options);

        assert!(options.parse_regular_expression);
        assert_eq!(
            options.preserve_parens,
            OxcParseOptions::default().preserve_parens
        );
        assert_eq!(
            options.enable_ident_hashes,
            OxcParseOptions::default().enable_ident_hashes
        );
    }

    #[test]
    fn test_resolved_parser_stage_options_defaults_to_telarel_values() {
        let options: ResolvedParserStageOptions =
            ResolvedParserStageOptions::default();

        assert!(!options.preserve_parens);
        assert!(!options.allow_return_outside_function);
        assert!(!options.oxc.preserve_parens);
        assert!(!options.oxc.allow_return_outside_function);
        assert!(options.oxc.enable_ident_hashes);
    }

    #[test]
    fn test_from_resolved_options_maps_every_field() {
        let resolved: ResolvedOptions = resolved_options();

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
}
