use std::path::PathBuf;

use oxc::codegen::{CommentOptions, IndentChar, LegalComment};

use crate::_types::options::OxcCodegenOptions;
use crate::_types::options::resolved::ResolvedOptions;
use crate::_types::options::sourcemap::SourceMapOptions;
use crate::helpers::resolve::resolve_generator_stage_options;

/// The indentation style for generated code.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum IndentStyle {
    /// Use spaces.
    #[default]
    Space,
    /// Use tabs.
    Tab,
}

impl From<IndentStyle> for IndentChar {
    fn from(style: IndentStyle) -> Self {
        match style {
            | IndentStyle::Space => IndentChar::Space,
            | IndentStyle::Tab => IndentChar::Tab,
        }
    }
}

impl From<IndentChar> for IndentStyle {
    fn from(indent_char: IndentChar) -> Self {
        match indent_char {
            | IndentChar::Space => IndentStyle::Space,
            | IndentChar::Tab => IndentStyle::Tab,
        }
    }
}

/// A partial patch of raw oxc comment options.
///
/// Only fields set to `Some` are applied. A default patch sets no category and
/// therefore changes nothing.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PartialOxcCommentOptions {
    /// Whether normal comments are printed.
    pub normal: Option<bool>,

    /// Whether JSDoc comments are printed.
    pub jsdoc: Option<bool>,

    /// Whether annotation comments are printed.
    pub annotation: Option<bool>,

    /// The legal-comment policy.
    pub legal: Option<LegalComment>,
}

impl PartialOxcCommentOptions {
    /// Whether no comment category is set.
    pub fn is_empty(&self) -> bool {
        self.normal.is_none()
            && self.jsdoc.is_none()
            && self.annotation.is_none()
            && self.legal.is_none()
    }

    /// Apply every set category to `options`. Unset categories leave `options`
    /// unchanged.
    pub fn apply_to(
        &self,
        options: &mut CommentOptions,
    ) {
        if let Some(value) = self.normal {
            options.normal = value;
        }

        if let Some(value) = self.jsdoc {
            options.jsdoc = value;
        }

        if let Some(value) = self.annotation {
            options.annotation = value;
        }

        if let Some(value) = &self.legal {
            options.legal = value.clone();
        }
    }
}

/// A partial patch of raw oxc codegen options.
///
/// Only fields set to `Some` are applied. A default patch sets no fields and
/// therefore changes nothing.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PartialOxcCodegenOptions {
    /// Whether string literals use single quotes.
    pub single_quote: Option<bool>,

    /// Whether generated code is minified.
    pub minify: Option<bool>,

    /// Whether non-ASCII characters are escaped.
    pub ascii_only: Option<bool>,

    /// A partial patch of comment categories.
    pub comments: Option<PartialOxcCommentOptions>,

    /// The indentation character.
    pub indent_char: Option<IndentChar>,

    /// The number of characters per indentation level.
    pub indent_width: Option<usize>,

    /// The initial indentation level.
    pub initial_indent: Option<u32>,
}

impl PartialOxcCodegenOptions {
    /// Apply every set field to `options`. Unset fields leave `options`
    /// unchanged.
    pub fn apply_to(
        &self,
        options: &mut OxcCodegenOptions,
    ) {
        if let Some(value) = self.single_quote {
            options.single_quote = value;
        }

        if let Some(value) = self.minify {
            options.minify = value;
        }

        if let Some(value) = self.ascii_only {
            options.ascii_only = value;
        }

        if let Some(comments) = &self.comments {
            comments.apply_to(&mut options.comments);
        }

        if let Some(value) = self.indent_char {
            options.indent_char = value;
        }

        if let Some(value) = self.indent_width {
            options.indent_width = value;
        }

        if let Some(value) = self.initial_indent {
            options.initial_indent = value;
        }
    }
}

/// Generator-stage options requested for a run.
#[derive(Debug, Clone, Default)]
pub struct GeneratorOptions {
    /// Whether string literals use single quotes instead of double quotes.
    ///
    /// By default, it is `false`.
    pub single_quote: Option<bool>,

    /// Whether non-ASCII characters are escaped in generated output.
    ///
    /// By default, it is `false`.
    pub ascii_only: Option<bool>,

    /// The indentation style for generated code.
    ///
    /// By default, it is `IndentStyle::Space`.
    pub indent_style: Option<IndentStyle>,

    /// The number of characters per indentation level.
    ///
    /// By default, it is `2`.
    pub indent_width: Option<usize>,

    /// Whether comments are printed in generated code.
    ///
    /// By default, it is `true`. When effective `minify` is `true`, comments
    /// are disabled only when neither this field nor a raw `oxc.comments`
    /// field is set.
    pub comments: Option<bool>,

    /// Whether generated code is minified.
    ///
    /// By default, it is `false`.
    pub minify: Option<bool>,

    /// A partial patch of raw oxc codegen options applied over the Telarel
    /// defaults.
    ///
    /// Only set fields override the defaults. A curated field wins over the
    /// matching raw field. An unset or empty patch changes nothing.
    /// `source_map_path` is owned by Telarel and is ignored here.
    pub oxc: Option<PartialOxcCodegenOptions>,
}

/// Effective generator-stage settings.
///
/// `oxc` is the full merged object and the curated fields mirror it.
#[derive(Debug, Clone)]
pub struct ResolvedGeneratorStageOptions {
    /// Whether string literals use single quotes.
    pub single_quote: bool,

    /// Whether non-ASCII characters are escaped.
    pub ascii_only: bool,

    /// The resolved indentation style.
    pub indent_style: IndentStyle,

    /// The resolved indentation width.
    pub indent_width: usize,

    /// Whether any comment category is printed.
    pub comments: bool,

    /// Whether generated code is minified.
    pub minify: bool,

    /// The effective options for the raw oxc codegen.
    pub oxc: OxcCodegenOptions,
}

impl Default for ResolvedGeneratorStageOptions {
    fn default() -> Self {
        resolve_generator_stage_options(None)
    }
}

/// Resolved options for the builtin [`crate::ast::generator::Generator`].
#[derive(Debug, Clone)]
pub struct ResolvedGeneratorOptions {
    /// Resolved working directory.
    pub cwd: PathBuf,
    /// The file being generated.
    pub file: String,
    /// The source-map inputs and outputs requested for the step.
    pub source_map: SourceMapOptions,
    /// Whether string literals use single quotes.
    pub single_quote: bool,
    /// Whether non-ASCII characters are escaped.
    pub ascii_only: bool,
    /// The resolved indentation style.
    pub indent_style: IndentStyle,
    /// The resolved indentation width.
    pub indent_width: usize,
    /// Whether any comment category is printed.
    pub comments: bool,
    /// Whether generated code is minified.
    pub minify: bool,
    /// The effective options for the raw oxc codegen.
    pub oxc: OxcCodegenOptions,
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
    use oxc::codegen::CommentOptions;

    use crate::_types::grammar::language::Language;
    use crate::_types::grammar::source_type::SourceType;
    use crate::_types::options::builtin::parser::ResolvedParserStageOptions;
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
    fn test_generator_options_default_is_all_unset() {
        let options: GeneratorOptions = GeneratorOptions::default();

        assert!(options.single_quote.is_none());
        assert!(options.ascii_only.is_none());
        assert!(options.indent_style.is_none());
        assert!(options.indent_width.is_none());
        assert!(options.comments.is_none());
        assert!(options.minify.is_none());
        assert!(options.oxc.is_none());
    }

    #[test]
    fn test_partial_oxc_comment_options_is_empty() {
        assert!(PartialOxcCommentOptions::default().is_empty());

        assert!(
            !PartialOxcCommentOptions {
                normal: Some(false),
                ..PartialOxcCommentOptions::default()
            }
            .is_empty()
        );
    }

    #[test]
    fn test_partial_oxc_comment_options_applies_only_set_categories() {
        let mut options: CommentOptions = CommentOptions::default();

        PartialOxcCommentOptions {
            jsdoc: Some(false),
            ..PartialOxcCommentOptions::default()
        }
        .apply_to(&mut options);

        assert!(options.normal);
        assert!(!options.jsdoc);
        assert!(options.annotation);
        assert_eq!(options.legal, LegalComment::Inline);
    }

    #[test]
    fn test_partial_oxc_codegen_options_default_is_a_no_op() {
        let baseline: OxcCodegenOptions = OxcCodegenOptions::default();
        let mut options: OxcCodegenOptions = OxcCodegenOptions::default();

        PartialOxcCodegenOptions::default().apply_to(&mut options);

        assert_eq!(options.single_quote, baseline.single_quote);
        assert_eq!(options.minify, baseline.minify);
        assert_eq!(options.ascii_only, baseline.ascii_only);
        assert_eq!(options.indent_char, baseline.indent_char);
        assert_eq!(options.indent_width, baseline.indent_width);
        assert_eq!(options.initial_indent, baseline.initial_indent);
        assert_eq!(options.comments, baseline.comments);
    }

    #[test]
    fn test_partial_oxc_codegen_options_applies_only_set_fields() {
        let mut options: OxcCodegenOptions = OxcCodegenOptions::default();

        PartialOxcCodegenOptions {
            indent_char: Some(IndentChar::Space),
            comments: Some(PartialOxcCommentOptions {
                normal: Some(false),
                ..PartialOxcCommentOptions::default()
            }),
            ..PartialOxcCodegenOptions::default()
        }
        .apply_to(&mut options);

        assert_eq!(options.indent_char, IndentChar::Space);
        assert!(!options.comments.normal);
        assert!(options.comments.jsdoc);
        assert_eq!(
            options.indent_width,
            OxcCodegenOptions::default().indent_width
        );
        assert!(!options.single_quote);
    }

    #[test]
    fn test_resolved_generator_stage_options_defaults_to_telarel_values() {
        let options: ResolvedGeneratorStageOptions =
            ResolvedGeneratorStageOptions::default();

        assert!(!options.single_quote);
        assert!(!options.ascii_only);
        assert_eq!(options.indent_style, IndentStyle::Space);
        assert_eq!(options.oxc.indent_char, IndentChar::Space);
        assert_eq!(options.indent_width, 2);
        assert_eq!(options.oxc.indent_width, 2);
        assert!(options.comments);
        assert!(options.oxc.comments.normal);
        assert!(options.oxc.comments.jsdoc);
        assert!(options.oxc.comments.annotation);
        assert_eq!(options.oxc.comments.legal, LegalComment::Inline);
        assert!(!options.minify);
    }

    #[test]
    fn test_indent_style_round_trips_through_indent_char() {
        assert_eq!(IndentStyle::from(IndentChar::Space), IndentStyle::Space);
        assert_eq!(IndentStyle::from(IndentChar::Tab), IndentStyle::Tab);
        assert_eq!(IndentChar::from(IndentStyle::Space), IndentChar::Space);
        assert_eq!(IndentChar::from(IndentStyle::Tab), IndentChar::Tab);
    }

    #[test]
    fn test_from_resolved_options_maps_every_field() {
        let resolved: ResolvedOptions = resolved_options();

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
