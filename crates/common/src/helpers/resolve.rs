use std::path::PathBuf;

use oxc::codegen::{CommentOptions, IndentChar};
use oxc::span::SourceType as OxcSourceType;

use crate::_types::grammar::language::{Language, grammar_source_type};
use crate::_types::grammar::source_type::{SourceType, with_module_kind};
use crate::_types::options::OxcCodegenOptions;
use crate::_types::options::OxcParseOptions;
use crate::_types::options::builtin::generator::{
    GeneratorOptions, IndentStyle, ResolvedGeneratorOptions,
    ResolvedGeneratorStageOptions,
};
use crate::_types::options::builtin::parser::{
    ParserOptions, ResolvedParserStageOptions,
};
use crate::_types::options::output::ResolvedOutputOptions;
use crate::_types::options::resolvable::ResolvableOptions;
use crate::_types::options::resolved::ResolvedOptions;
use crate::_types::options::sourcemap::SourceMapOptions;
use crate::helpers::path::{get_process_cwd, strip_file_suffix};

/// The file name used when the caller does not supply one.
pub const FILE_DEFAULT: &str = "index.ts";

/// Infer the oxc source type from a file, falling back to the default
/// (an ES module) for extensionless or unknown files.
pub fn source_type_for_module_id(module_id: &str) -> OxcSourceType {
    let path: &str = strip_file_suffix(module_id);

    OxcSourceType::from_path(path).unwrap_or_default()
}

/// The oxc grammar for a file. An explicit language wins. Otherwise, infer
/// the grammar from the file.
pub fn resolve_grammar(
    file: &str,
    language: Option<Language>,
) -> OxcSourceType {
    match language {
        | Some(language) => grammar_source_type(language),
        | None => source_type_for_module_id(file),
    }
}

/// Narrow an oxc source type to an internal grammar.
pub fn from_oxc_language(source_type: OxcSourceType) -> Language {
    match (source_type.is_typescript(), source_type.is_jsx()) {
        | (true, true) => Language::TSX,
        | (true, false) => {
            if source_type.is_typescript_definition() {
                Language::DTS
            } else {
                Language::TS
            }
        },
        | (false, true) => Language::JSX,
        | (false, false) => Language::JS,
    }
}

/// Infer the telarel grammar for a file.
pub fn infer_language(
    file: &str,
    language: Option<Language>,
) -> Language {
    from_oxc_language(resolve_grammar(file, language))
}

/// Narrow an oxc source type to an internal module kind.
pub fn from_oxc_module_kind(source_type: OxcSourceType) -> SourceType {
    if source_type.is_script() {
        SourceType::Script
    } else if source_type.is_commonjs() {
        SourceType::CommonJS
    } else if source_type.is_module() {
        SourceType::Module
    } else {
        SourceType::Unambiguous
    }
}

/// Resolve the telarel source type for a file: the resolved grammar's module kind,
/// overlaid by an explicit option.
pub fn resolve_source_type(
    file: &str,
    language: Option<Language>,
    source_type: Option<SourceType>,
) -> SourceType {
    let grammar: OxcSourceType = resolve_grammar(file, language);

    let resolved: OxcSourceType = match source_type {
        | Some(module_kind) => with_module_kind(grammar, module_kind),
        | None => grammar,
    };

    from_oxc_module_kind(resolved)
}

/// The curated generator fields that override the effective oxc object.
#[derive(Debug, Clone, Copy, Default)]
pub struct CuratedGeneratorOptions {
    single_quote: Option<bool>,
    ascii_only: Option<bool>,
    indent_style: Option<IndentStyle>,
    indent_width: Option<usize>,
    comments: Option<bool>,
    minify: Option<bool>,
}

impl From<&GeneratorOptions> for CuratedGeneratorOptions {
    fn from(options: &GeneratorOptions) -> Self {
        Self {
            single_quote: options.single_quote,
            ascii_only: options.ascii_only,
            indent_style: options.indent_style,
            indent_width: options.indent_width,
            comments: options.comments,
            minify: options.minify,
        }
    }
}

impl From<&ResolvedGeneratorOptions> for CuratedGeneratorOptions {
    fn from(options: &ResolvedGeneratorOptions) -> Self {
        Self {
            single_quote: Some(options.single_quote),
            ascii_only: Some(options.ascii_only),
            indent_style: Some(options.indent_style),
            indent_width: Some(options.indent_width),
            comments: Some(options.comments),
            minify: Some(options.minify),
        }
    }
}

/// Overlay the curated parser fields onto `oxc`.
///
/// The overlay is idempotent: applying it to an already-resolved object leaves
/// that object unchanged.
pub fn apply_curated_parser_options(
    oxc: &mut OxcParseOptions,
    preserve_parens: Option<bool>,
    allow_return_outside_function: Option<bool>,
) {
    if let Some(value) = preserve_parens {
        oxc.preserve_parens = value;
    }

    if let Some(value) = allow_return_outside_function {
        oxc.allow_return_outside_function = value;
    }
}

/// Overlay the curated generator fields onto `oxc`.
///
/// The overlay is idempotent: applying it to an already-resolved object leaves
/// that object unchanged. Curated `comments: false` disables every category.
/// Curated `comments: true` restores the default policy only when the effective
/// categories are all disabled, so explicitly set raw categories survive.
pub fn apply_curated_generator_options(
    oxc: &mut OxcCodegenOptions,
    curated: CuratedGeneratorOptions,
) {
    if let Some(value) = curated.single_quote {
        oxc.single_quote = value;
    }

    if let Some(value) = curated.ascii_only {
        oxc.ascii_only = value;
    }

    if let Some(value) = curated.indent_style {
        oxc.indent_char = IndentChar::from(value);
    }

    if let Some(value) = curated.indent_width {
        oxc.indent_width = value;
    }

    if let Some(value) = curated.minify {
        oxc.minify = value;
    }

    match curated.comments {
        | Some(true) => {
            if oxc.comments == CommentOptions::disabled() {
                oxc.comments = CommentOptions::default();
            }
        },
        | Some(false) => oxc.comments = CommentOptions::disabled(),
        | None => {},
    }
}

/// Resolve the parser stage: the Telarel defaults are the base, the raw `oxc`
/// patch overrides them, and curated fields win over both.
pub fn resolve_parser_stage_options(
    options: Option<&ParserOptions>
) -> ResolvedParserStageOptions {
    let mut oxc: OxcParseOptions = ResolvedParserStageOptions::default().oxc;

    if let Some(raw) = options.and_then(|options| options.oxc.as_ref()) {
        raw.apply_to(&mut oxc);
    }

    apply_curated_parser_options(
        &mut oxc,
        options.and_then(|options| options.preserve_parens),
        options.and_then(|options| options.allow_return_outside_function),
    );

    ResolvedParserStageOptions::from_oxc(oxc)
}

/// Resolve the generator stage: the Telarel defaults are the base, the raw
/// `oxc` patch overrides them, and curated fields win over both.
///
/// An effective `minify` disables comments unless the raw patch sets comment
/// categories explicitly. Curated comment settings follow the documented
/// precedence.
pub fn resolve_generator_stage_options(
    options: Option<&GeneratorOptions>
) -> ResolvedGeneratorStageOptions {
    let mut oxc: OxcCodegenOptions =
        ResolvedGeneratorStageOptions::default().oxc;

    let raw = options.and_then(|options| options.oxc.as_ref());

    if let Some(raw) = raw {
        raw.apply_to(&mut oxc);
    }

    let curated: CuratedGeneratorOptions =
        options.map(CuratedGeneratorOptions::from).unwrap_or_default();

    apply_curated_generator_options(&mut oxc, curated);

    let raw_comments_explicit: bool = raw
        .and_then(|raw| raw.comments.as_ref())
        .is_some_and(|comments| !comments.is_empty());

    if curated.comments.is_none() && oxc.minify && !raw_comments_explicit {
        oxc.comments = CommentOptions::disabled();
    }

    // Telarel owns the map file name. The builtin generator sets it from
    // `file` at generation time.
    oxc.source_map_path = None;

    ResolvedGeneratorStageOptions::from_oxc(oxc)
}

/// Resolve the optional run settings into concrete options.
pub fn resolve_options(
    options: ResolvableOptions,
    plugin_names: Vec<String>,
) -> ResolvedOptions {
    let parser: ResolvedParserStageOptions =
        resolve_parser_stage_options(options.parser.as_ref());

    let generator: ResolvedGeneratorStageOptions =
        resolve_generator_stage_options(options.generator.as_ref());

    let cwd: PathBuf = options.cwd.unwrap_or_else(get_process_cwd);

    let file: String = options
        .file
        .filter(|file: &String| !file.is_empty())
        .unwrap_or_else(|| String::from(FILE_DEFAULT));

    let language: Language = infer_language(&file, options.language);

    let source_type: SourceType =
        resolve_source_type(&file, options.language, options.source_type);

    let source_map: SourceMapOptions = options.source_map.unwrap_or_default();

    let output_file: String = options
        .output
        .and_then(|output| output.file)
        .filter(|file: &String| !file.is_empty())
        .unwrap_or_else(|| file.clone());

    ResolvedOptions {
        cwd,
        file,
        language,
        source_type,
        parser,
        generator,
        output: ResolvedOutputOptions { file: output_file },
        source_map,
        plugins: plugin_names,
    }
}

#[cfg(test)]
mod tests {
    use crate::_types::options::builtin::generator::{
        PartialOxcCodegenOptions, PartialOxcCommentOptions,
    };
    use crate::_types::options::builtin::parser::PartialOxcParseOptions;
    use crate::_types::options::output::OutputOptions;
    use crate::_types::options::sourcemap::input::{
        InputSourceMap, InputSourceMapMode,
    };
    use crate::_types::options::sourcemap::output::{
        OutputSourceMap, OutputSourceMapMode,
    };

    use super::*;

    fn resolvable(
        file: Option<&str>,
        language: Option<Language>,
        source_type: Option<SourceType>,
    ) -> ResolvableOptions {
        ResolvableOptions {
            file: file.map(String::from),
            language,
            source_type,
            ..ResolvableOptions::default()
        }
    }

    #[test]
    fn test_from_oxc_language_narrows_grammar() {
        assert_eq!(from_oxc_language(OxcSourceType::ts()), Language::TS);
        assert_eq!(from_oxc_language(OxcSourceType::d_ts()), Language::DTS);
        assert_eq!(from_oxc_language(OxcSourceType::jsx()), Language::JSX);
        assert_eq!(from_oxc_language(OxcSourceType::tsx()), Language::TSX);
        assert_eq!(from_oxc_language(OxcSourceType::default()), Language::JS);
    }

    #[test]
    fn test_infer_language_defaults_to_typescript_for_ts_file() {
        assert_eq!(infer_language("index.ts", None), Language::TS);
    }

    #[test]
    fn test_infer_language_defaults_to_js_for_js_file() {
        assert_eq!(infer_language("index.js", None), Language::JS);
    }

    #[test]
    fn test_infer_language_explicit_wins() {
        assert_eq!(
            infer_language("index.js", Some(Language::TS)),
            Language::TS
        );
    }

    #[test]
    fn test_infer_language_tsx_detected() {
        assert_eq!(infer_language("index.tsx", None), Language::TSX);
    }

    #[test]
    fn test_infer_language_dts_detected() {
        assert_eq!(infer_language("index.d.ts", None), Language::DTS);
    }

    #[test]
    fn test_infer_language_jsx_detected() {
        assert_eq!(infer_language("index.jsx", None), Language::JSX);
    }

    #[test]
    fn test_infer_language_strips_query_suffix() {
        assert_eq!(infer_language("index.js?v=d48eb52a", None), Language::JS);
    }

    #[test]
    fn test_infer_language_strips_hash_suffix() {
        assert_eq!(infer_language("index.ts#hash", None), Language::TS);
    }

    #[test]
    fn test_infer_language_strips_query_and_hash() {
        assert_eq!(infer_language("index.tsx?v=1#h", None), Language::TSX);
    }

    #[test]
    fn test_from_oxc_module_kind_narrows_module_kind() {
        let script: OxcSourceType = OxcSourceType::default().with_script(true);
        assert_eq!(from_oxc_module_kind(script), SourceType::Script);

        let commonjs: OxcSourceType =
            OxcSourceType::default().with_commonjs(true);
        assert_eq!(from_oxc_module_kind(commonjs), SourceType::CommonJS);

        let module: OxcSourceType = OxcSourceType::default().with_module(true);
        assert_eq!(from_oxc_module_kind(module), SourceType::Module);

        assert_eq!(
            from_oxc_module_kind(OxcSourceType::unambiguous()),
            SourceType::Unambiguous,
        );
    }

    #[test]
    fn test_resolve_source_type_keeps_grammar_module_kind_without_option() {
        // An explicit language yields an unambiguous grammar: its module
        // kind is telarel's `Unambiguous`, mirrored from the parser.
        assert_eq!(
            resolve_source_type("index.ts", Some(Language::TS), None),
            SourceType::Unambiguous,
        );

        assert_eq!(
            resolve_source_type("index.ts", None, None),
            SourceType::Unambiguous,
        );

        assert_eq!(
            resolve_source_type("index.mjs", None, None),
            SourceType::Module,
        );
    }

    #[test]
    fn test_resolve_source_type_explicit_module_kind_wins() {
        assert_eq!(
            resolve_source_type("index.ts", None, Some(SourceType::Script)),
            SourceType::Script,
        );
    }

    #[test]
    fn test_resolve_source_type_cjs_extension_resolves_commonjs() {
        assert_eq!(infer_language("index.cjs", None), Language::JS);

        assert_eq!(
            resolve_source_type("index.cjs", None, None),
            SourceType::CommonJS,
        );
    }

    #[test]
    fn test_resolve_options_explicit_fields_pass_through() {
        let options: ResolvableOptions = ResolvableOptions {
            cwd: Some(PathBuf::from("/repo")),
            file: Some(String::from("src/app.tsx")),
            language: Some(Language::TSX),
            source_type: Some(SourceType::Script),
            output: Some(OutputOptions {
                file: Some(String::from("dist/app.js")),
            }),
            source_map: Some(SourceMapOptions::default()),
            ..ResolvableOptions::default()
        };

        let resolved: ResolvedOptions =
            resolve_options(options, vec![String::from("probe")]);

        assert_eq!(resolved.cwd, PathBuf::from("/repo"));
        assert_eq!(resolved.file, "src/app.tsx");
        assert_eq!(resolved.language, Language::TSX);
        assert_eq!(resolved.source_type, SourceType::Script);
        assert_eq!(resolved.output.file, "dist/app.js");
        assert!(resolved.source_map.input.is_none());
        assert!(resolved.source_map.output.is_none());
        assert_eq!(resolved.plugins, vec![String::from("probe")]);
    }

    #[test]
    fn test_resolve_options_output_file_defaults_to_file() {
        let resolved: ResolvedOptions = resolve_options(
            ResolvableOptions {
                file: Some(String::from("src/app.ts")),
                ..ResolvableOptions::default()
            },
            vec![],
        );

        assert_eq!(resolved.output.file, "src/app.ts");
    }

    #[test]
    fn test_resolve_options_output_file_empty_falls_back_to_file() {
        let resolved: ResolvedOptions = resolve_options(
            ResolvableOptions {
                file: Some(String::from("src/app.ts")),
                output: Some(OutputOptions { file: Some(String::new()) }),
                ..ResolvableOptions::default()
            },
            vec![],
        );

        assert_eq!(resolved.output.file, "src/app.ts");
    }

    #[test]
    fn test_resolve_options_empty_file_falls_back_to_index_ts() {
        let resolved: ResolvedOptions = resolve_options(
            ResolvableOptions {
                file: Some(String::new()),
                ..ResolvableOptions::default()
            },
            vec![],
        );

        assert_eq!(resolved.file, "index.ts");
        assert_eq!(resolved.language, Language::TS);
        assert_eq!(resolved.source_type, SourceType::Unambiguous);
        assert_eq!(resolved.output.file, "index.ts");
    }

    #[test]
    fn test_resolve_options_empty_file_and_output_file_fall_back_to_index_ts() {
        let resolved: ResolvedOptions = resolve_options(
            ResolvableOptions {
                file: Some(String::new()),
                output: Some(OutputOptions { file: Some(String::new()) }),
                ..ResolvableOptions::default()
            },
            vec![],
        );

        assert_eq!(resolved.file, "index.ts");
        assert_eq!(resolved.output.file, "index.ts");
    }

    #[test]
    fn test_resolve_options_explicit_output_file_wins_over_empty_file() {
        let resolved: ResolvedOptions = resolve_options(
            ResolvableOptions {
                file: Some(String::new()),
                output: Some(OutputOptions {
                    file: Some(String::from("dist/app.js")),
                }),
                ..ResolvableOptions::default()
            },
            vec![],
        );

        assert_eq!(resolved.file, "index.ts");
        assert_eq!(resolved.output.file, "dist/app.js");
    }

    #[test]
    fn test_resolve_options_defaults_file_to_index_ts_and_infers_ts() {
        let resolved: ResolvedOptions =
            resolve_options(ResolvableOptions::default(), vec![]);

        assert_eq!(resolved.file, "index.ts");
        assert_eq!(resolved.language, Language::TS);
        assert_eq!(resolved.source_type, SourceType::Unambiguous);
    }

    #[test]
    fn test_resolve_options_explicit_language_and_source_type_win() {
        let options: ResolvableOptions = resolvable(
            Some("index.js"),
            Some(Language::TS),
            Some(SourceType::Script),
        );

        let resolved: ResolvedOptions = resolve_options(options, vec![]);

        assert_eq!(resolved.file, "index.js");
        assert_eq!(resolved.language, Language::TS);
        assert_eq!(resolved.source_type, SourceType::Script);
    }

    #[test]
    fn test_resolve_options_source_map_defaults_to_off() {
        let resolved: ResolvedOptions =
            resolve_options(ResolvableOptions::default(), vec![]);

        assert!(resolved.source_map.input.is_none());
        assert!(resolved.source_map.output.is_none());
    }

    #[test]
    fn test_resolve_options_keeps_explicit_source_map() {
        let source_map: SourceMapOptions = SourceMapOptions {
            input: Some(InputSourceMap::Mode(InputSourceMapMode::Inline)),
            output: Some(OutputSourceMap::Mode(OutputSourceMapMode::Both)),
        };

        let options: ResolvableOptions = ResolvableOptions {
            source_map: Some(source_map),
            ..ResolvableOptions::default()
        };

        let resolved: ResolvedOptions = resolve_options(options, vec![]);

        assert!(matches!(
            resolved.source_map.input,
            Some(InputSourceMap::Mode(InputSourceMapMode::Inline)),
        ));
        assert!(matches!(
            resolved.source_map.output,
            Some(OutputSourceMap::Mode(OutputSourceMapMode::Both)),
        ));
    }

    #[test]
    fn test_resolve_options_cwd_defaults_to_process_cwd() {
        let resolved: ResolvedOptions =
            resolve_options(ResolvableOptions::default(), vec![]);

        let expected: PathBuf =
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));

        assert_eq!(resolved.cwd, expected);
    }

    #[test]
    fn test_resolve_options_keeps_plugins_in_order_with_duplicates() {
        let resolved: ResolvedOptions = resolve_options(
            ResolvableOptions::default(),
            vec![
                String::from("first"),
                String::from("second"),
                String::from("first"),
            ],
        );

        assert_eq!(resolved.plugins, vec!["first", "second", "first"]);
    }

    #[test]
    fn test_resolve_parser_stage_options_applies_telarel_defaults() {
        let resolved: ResolvedParserStageOptions =
            resolve_parser_stage_options(None);

        assert!(!resolved.preserve_parens);
        assert!(!resolved.allow_return_outside_function);
        assert!(!resolved.oxc.preserve_parens);
        assert!(!resolved.oxc.allow_return_outside_function);
        assert!(resolved.oxc.enable_ident_hashes);
    }

    #[test]
    fn test_resolved_parser_stage_options_default_matches_resolver() {
        // The documented Telarel override over oxc's raw defaults.
        let expected_oxc: OxcParseOptions = OxcParseOptions {
            preserve_parens: false,
            ..OxcParseOptions::default()
        };

        let default: ResolvedParserStageOptions =
            ResolvedParserStageOptions::default();
        let resolved: ResolvedParserStageOptions =
            resolve_parser_stage_options(None);

        for oxc in [&default.oxc, &resolved.oxc] {
            assert_eq!(
                oxc.parse_regular_expression,
                expected_oxc.parse_regular_expression
            );
            assert_eq!(
                oxc.allow_return_outside_function,
                expected_oxc.allow_return_outside_function
            );
            assert_eq!(oxc.preserve_parens, expected_oxc.preserve_parens);
            assert_eq!(
                oxc.allow_v8_intrinsics,
                expected_oxc.allow_v8_intrinsics
            );
            assert_eq!(
                oxc.enable_ident_hashes,
                expected_oxc.enable_ident_hashes
            );
        }

        for stage in [&default, &resolved] {
            assert_eq!(stage.preserve_parens, expected_oxc.preserve_parens);
            assert_eq!(
                stage.allow_return_outside_function,
                expected_oxc.allow_return_outside_function
            );
        }

        assert_eq!(default.preserve_parens, resolved.preserve_parens);
        assert_eq!(
            default.allow_return_outside_function,
            resolved.allow_return_outside_function
        );
        assert_eq!(
            default.oxc.parse_regular_expression,
            resolved.oxc.parse_regular_expression
        );
        assert_eq!(
            default.oxc.allow_return_outside_function,
            resolved.oxc.allow_return_outside_function
        );
        assert_eq!(default.oxc.preserve_parens, resolved.oxc.preserve_parens);
        assert_eq!(
            default.oxc.allow_v8_intrinsics,
            resolved.oxc.allow_v8_intrinsics
        );
        assert_eq!(
            default.oxc.enable_ident_hashes,
            resolved.oxc.enable_ident_hashes
        );
    }

    #[test]
    fn test_resolve_parser_stage_options_raw_partial_keeps_telarel_defaults() {
        let resolved: ResolvedParserStageOptions =
            resolve_parser_stage_options(Some(&ParserOptions {
                oxc: Some(PartialOxcParseOptions {
                    parse_regular_expression: Some(true),
                    ..PartialOxcParseOptions::default()
                }),
                ..ParserOptions::default()
            }));

        assert!(resolved.oxc.parse_regular_expression);
        assert!(!resolved.preserve_parens);
        assert!(!resolved.oxc.preserve_parens);
        assert!(!resolved.allow_return_outside_function);
        assert!(resolved.oxc.enable_ident_hashes);
    }

    #[test]
    fn test_resolve_parser_stage_options_raw_preserve_parens_wins() {
        let resolved: ResolvedParserStageOptions =
            resolve_parser_stage_options(Some(&ParserOptions {
                oxc: Some(PartialOxcParseOptions {
                    preserve_parens: Some(true),
                    ..PartialOxcParseOptions::default()
                }),
                ..ParserOptions::default()
            }));

        assert!(resolved.preserve_parens);
        assert!(resolved.oxc.preserve_parens);
    }

    #[test]
    fn test_resolve_parser_stage_options_curated_overrides_raw() {
        let resolved: ResolvedParserStageOptions =
            resolve_parser_stage_options(Some(&ParserOptions {
                preserve_parens: Some(true),
                allow_return_outside_function: Some(true),
                oxc: Some(PartialOxcParseOptions {
                    preserve_parens: Some(false),
                    allow_return_outside_function: Some(false),
                    ..PartialOxcParseOptions::default()
                }),
            }));

        assert!(resolved.preserve_parens);
        assert!(resolved.allow_return_outside_function);
        assert!(resolved.oxc.preserve_parens);
        assert!(resolved.oxc.allow_return_outside_function);
    }

    #[test]
    fn test_resolve_parser_stage_options_empty_patch_is_a_no_op() {
        let base: ResolvedParserStageOptions =
            resolve_parser_stage_options(None);

        let resolved: ResolvedParserStageOptions =
            resolve_parser_stage_options(Some(&ParserOptions {
                oxc: Some(PartialOxcParseOptions::default()),
                ..ParserOptions::default()
            }));

        assert_eq!(resolved.preserve_parens, base.preserve_parens);
        assert_eq!(
            resolved.allow_return_outside_function,
            base.allow_return_outside_function
        );
        assert_eq!(
            resolved.oxc.parse_regular_expression,
            base.oxc.parse_regular_expression
        );
        assert_eq!(resolved.oxc.preserve_parens, base.oxc.preserve_parens);
        assert_eq!(
            resolved.oxc.allow_return_outside_function,
            base.oxc.allow_return_outside_function
        );
        assert_eq!(
            resolved.oxc.allow_v8_intrinsics,
            base.oxc.allow_v8_intrinsics
        );
        assert_eq!(
            resolved.oxc.enable_ident_hashes,
            base.oxc.enable_ident_hashes
        );
    }

    #[test]
    fn test_resolve_generator_stage_options_applies_telarel_defaults() {
        let resolved: ResolvedGeneratorStageOptions =
            resolve_generator_stage_options(None);

        assert!(!resolved.single_quote);
        assert!(!resolved.ascii_only);
        assert_eq!(resolved.indent_style, IndentStyle::Space);
        assert_eq!(resolved.oxc.indent_char, IndentChar::Space);
        assert_eq!(resolved.indent_width, 2);
        assert_eq!(resolved.oxc.indent_width, 2);
        assert!(resolved.comments);
        assert!(resolved.oxc.comments.normal);
        assert!(resolved.oxc.comments.jsdoc);
        assert!(resolved.oxc.comments.annotation);
        assert_eq!(
            resolved.oxc.comments.legal,
            oxc::codegen::LegalComment::Inline
        );
        assert!(!resolved.minify);
        assert_eq!(resolved.oxc.initial_indent, 0);
        assert!(resolved.oxc.source_map_path.is_none());
    }

    #[test]
    fn test_resolved_generator_stage_options_default_matches_resolver() {
        // The documented Telarel overrides over oxc's raw defaults, including
        // the resolver's cleared map path.
        let expected_oxc: OxcCodegenOptions = OxcCodegenOptions {
            indent_char: IndentChar::Space,
            indent_width: 2,
            source_map_path: None,
            ..OxcCodegenOptions::default()
        };

        let default: ResolvedGeneratorStageOptions =
            ResolvedGeneratorStageOptions::default();
        let resolved: ResolvedGeneratorStageOptions =
            resolve_generator_stage_options(None);

        for oxc in [&default.oxc, &resolved.oxc] {
            assert_eq!(oxc.single_quote, expected_oxc.single_quote);
            assert_eq!(oxc.minify, expected_oxc.minify);
            assert_eq!(oxc.ascii_only, expected_oxc.ascii_only);
            assert_eq!(oxc.comments, expected_oxc.comments);
            assert_eq!(oxc.source_map_path, expected_oxc.source_map_path);
            assert_eq!(oxc.indent_char, expected_oxc.indent_char);
            assert_eq!(oxc.indent_width, expected_oxc.indent_width);
            assert_eq!(oxc.initial_indent, expected_oxc.initial_indent);
        }

        for stage in [&default, &resolved] {
            assert_eq!(stage.single_quote, expected_oxc.single_quote);
            assert_eq!(stage.ascii_only, expected_oxc.ascii_only);
            assert_eq!(
                stage.indent_style,
                IndentStyle::from(expected_oxc.indent_char)
            );
            assert_eq!(stage.indent_width, expected_oxc.indent_width);
            assert_eq!(
                stage.comments,
                expected_oxc.comments != CommentOptions::disabled()
            );
            assert_eq!(stage.minify, expected_oxc.minify);
        }

        assert_eq!(default.single_quote, resolved.single_quote);
        assert_eq!(default.ascii_only, resolved.ascii_only);
        assert_eq!(default.indent_style, resolved.indent_style);
        assert_eq!(default.indent_width, resolved.indent_width);
        assert_eq!(default.comments, resolved.comments);
        assert_eq!(default.minify, resolved.minify);
        assert_eq!(default.oxc.single_quote, resolved.oxc.single_quote);
        assert_eq!(default.oxc.minify, resolved.oxc.minify);
        assert_eq!(default.oxc.ascii_only, resolved.oxc.ascii_only);
        assert_eq!(default.oxc.comments, resolved.oxc.comments);
        assert_eq!(default.oxc.source_map_path, resolved.oxc.source_map_path);
        assert_eq!(default.oxc.indent_char, resolved.oxc.indent_char);
        assert_eq!(default.oxc.indent_width, resolved.oxc.indent_width);
        assert_eq!(default.oxc.initial_indent, resolved.oxc.initial_indent);
    }

    #[test]
    fn test_resolve_generator_stage_options_raw_indent_char_keeps_width() {
        let resolved: ResolvedGeneratorStageOptions =
            resolve_generator_stage_options(Some(&GeneratorOptions {
                oxc: Some(PartialOxcCodegenOptions {
                    indent_char: Some(IndentChar::Tab),
                    ..PartialOxcCodegenOptions::default()
                }),
                ..GeneratorOptions::default()
            }));

        assert_eq!(resolved.indent_style, IndentStyle::Tab);
        assert_eq!(resolved.oxc.indent_char, IndentChar::Tab);
        assert_eq!(resolved.indent_width, 2);
        assert_eq!(resolved.oxc.indent_width, 2);
    }

    #[test]
    fn test_resolve_generator_stage_options_raw_indent_width_wins() {
        let resolved: ResolvedGeneratorStageOptions =
            resolve_generator_stage_options(Some(&GeneratorOptions {
                oxc: Some(PartialOxcCodegenOptions {
                    indent_width: Some(1),
                    ..PartialOxcCodegenOptions::default()
                }),
                ..GeneratorOptions::default()
            }));

        assert_eq!(resolved.indent_style, IndentStyle::Space);
        assert_eq!(resolved.indent_width, 1);
        assert_eq!(resolved.oxc.indent_width, 1);
    }

    #[test]
    fn test_resolve_generator_stage_options_curated_overrides_raw() {
        let resolved: ResolvedGeneratorStageOptions =
            resolve_generator_stage_options(Some(&GeneratorOptions {
                single_quote: Some(false),
                ascii_only: Some(false),
                indent_style: Some(IndentStyle::Space),
                indent_width: Some(4),
                minify: Some(false),
                oxc: Some(PartialOxcCodegenOptions {
                    single_quote: Some(true),
                    ascii_only: Some(true),
                    indent_char: Some(IndentChar::Tab),
                    indent_width: Some(1),
                    minify: Some(true),
                    ..PartialOxcCodegenOptions::default()
                }),
                ..GeneratorOptions::default()
            }));

        assert!(!resolved.single_quote);
        assert!(!resolved.ascii_only);
        assert_eq!(resolved.indent_style, IndentStyle::Space);
        assert_eq!(resolved.indent_width, 4);
        assert!(!resolved.minify);
        assert_eq!(resolved.oxc.indent_width, 4);
        assert_eq!(resolved.oxc.indent_char, IndentChar::Space);
    }

    #[test]
    fn test_resolve_generator_stage_options_empty_patch_is_a_no_op() {
        let base: ResolvedGeneratorStageOptions =
            resolve_generator_stage_options(None);

        let resolved: ResolvedGeneratorStageOptions =
            resolve_generator_stage_options(Some(&GeneratorOptions {
                oxc: Some(PartialOxcCodegenOptions::default()),
                ..GeneratorOptions::default()
            }));

        assert_eq!(resolved.single_quote, base.single_quote);
        assert_eq!(resolved.ascii_only, base.ascii_only);
        assert_eq!(resolved.indent_style, base.indent_style);
        assert_eq!(resolved.indent_width, base.indent_width);
        assert_eq!(resolved.comments, base.comments);
        assert_eq!(resolved.minify, base.minify);
        assert_eq!(resolved.oxc.indent_char, base.oxc.indent_char);
        assert_eq!(resolved.oxc.indent_width, base.oxc.indent_width);
        assert_eq!(resolved.oxc.initial_indent, base.oxc.initial_indent);
        assert_eq!(resolved.oxc.comments, base.oxc.comments);
    }

    #[test]
    fn test_resolve_generator_stage_options_minify_disables_comments_by_default()
     {
        let resolved: ResolvedGeneratorStageOptions =
            resolve_generator_stage_options(Some(&GeneratorOptions {
                minify: Some(true),
                ..GeneratorOptions::default()
            }));

        assert!(resolved.minify);
        assert!(!resolved.comments);
        assert!(!resolved.oxc.comments.normal);
        assert!(!resolved.oxc.comments.jsdoc);
        assert!(!resolved.oxc.comments.annotation);
        assert_eq!(
            resolved.oxc.comments.legal,
            oxc::codegen::LegalComment::None
        );
    }

    #[test]
    fn test_resolve_generator_stage_options_raw_minify_disables_comments() {
        let resolved: ResolvedGeneratorStageOptions =
            resolve_generator_stage_options(Some(&GeneratorOptions {
                oxc: Some(PartialOxcCodegenOptions {
                    minify: Some(true),
                    ..PartialOxcCodegenOptions::default()
                }),
                ..GeneratorOptions::default()
            }));

        assert!(resolved.minify);
        assert!(!resolved.comments);
        assert_eq!(resolved.oxc.comments, CommentOptions::disabled());
    }

    #[test]
    fn test_resolve_generator_stage_options_explicit_comments_win_over_minify()
    {
        let resolved: ResolvedGeneratorStageOptions =
            resolve_generator_stage_options(Some(&GeneratorOptions {
                comments: Some(true),
                minify: Some(true),
                ..GeneratorOptions::default()
            }));

        assert!(resolved.minify);
        assert!(resolved.comments);
        assert!(resolved.oxc.comments.normal);
        assert!(resolved.oxc.comments.jsdoc);
        assert!(resolved.oxc.comments.annotation);
    }

    #[test]
    fn test_resolve_generator_stage_options_comments_false_disables_all() {
        let resolved: ResolvedGeneratorStageOptions =
            resolve_generator_stage_options(Some(&GeneratorOptions {
                comments: Some(false),
                ..GeneratorOptions::default()
            }));

        assert!(!resolved.comments);
        assert_eq!(resolved.oxc.comments, CommentOptions::disabled());
    }

    #[test]
    fn test_resolve_generator_stage_options_comments_false_wins_over_minify() {
        let resolved: ResolvedGeneratorStageOptions =
            resolve_generator_stage_options(Some(&GeneratorOptions {
                comments: Some(false),
                minify: Some(true),
                ..GeneratorOptions::default()
            }));

        assert!(resolved.minify);
        assert!(!resolved.comments);
        assert_eq!(resolved.oxc.comments, CommentOptions::disabled());
    }

    #[test]
    fn test_resolve_generator_stage_options_comments_false_wins_over_raw_categories()
     {
        let resolved: ResolvedGeneratorStageOptions =
            resolve_generator_stage_options(Some(&GeneratorOptions {
                comments: Some(false),
                oxc: Some(PartialOxcCodegenOptions {
                    comments: Some(PartialOxcCommentOptions {
                        normal: Some(true),
                        jsdoc: Some(true),
                        annotation: Some(true),
                        legal: Some(oxc::codegen::LegalComment::Inline),
                    }),
                    ..PartialOxcCodegenOptions::default()
                }),
                ..GeneratorOptions::default()
            }));

        assert!(!resolved.comments);
        assert_eq!(resolved.oxc.comments, CommentOptions::disabled());
    }

    #[test]
    fn test_resolve_generator_stage_options_comments_true_keeps_comments() {
        let resolved: ResolvedGeneratorStageOptions =
            resolve_generator_stage_options(Some(&GeneratorOptions {
                comments: Some(true),
                minify: Some(false),
                ..GeneratorOptions::default()
            }));

        assert!(!resolved.minify);
        assert!(resolved.comments);
        assert!(resolved.oxc.comments.normal);
        assert!(resolved.oxc.comments.jsdoc);
        assert!(resolved.oxc.comments.annotation);
    }

    #[test]
    fn test_resolve_generator_stage_options_keeps_raw_comments_when_not_minified()
     {
        let resolved: ResolvedGeneratorStageOptions =
            resolve_generator_stage_options(Some(&GeneratorOptions {
                oxc: Some(PartialOxcCodegenOptions {
                    comments: Some(PartialOxcCommentOptions {
                        normal: Some(true),
                        jsdoc: Some(false),
                        annotation: Some(true),
                        legal: Some(oxc::codegen::LegalComment::None),
                    }),
                    ..PartialOxcCodegenOptions::default()
                }),
                ..GeneratorOptions::default()
            }));

        assert!(!resolved.minify);
        assert!(resolved.comments);
        assert!(resolved.oxc.comments.normal);
        assert!(!resolved.oxc.comments.jsdoc);
        assert!(resolved.oxc.comments.annotation);
        assert_eq!(
            resolved.oxc.comments.legal,
            oxc::codegen::LegalComment::None
        );
    }

    #[test]
    fn test_resolve_generator_stage_options_raw_comments_survive_minify() {
        let resolved: ResolvedGeneratorStageOptions =
            resolve_generator_stage_options(Some(&GeneratorOptions {
                oxc: Some(PartialOxcCodegenOptions {
                    minify: Some(true),
                    comments: Some(PartialOxcCommentOptions {
                        normal: Some(true),
                        ..PartialOxcCommentOptions::default()
                    }),
                    ..PartialOxcCodegenOptions::default()
                }),
                ..GeneratorOptions::default()
            }));

        assert!(resolved.minify);
        assert!(resolved.comments);
        assert!(resolved.oxc.comments.normal);
        assert!(resolved.oxc.comments.jsdoc);
        assert!(resolved.oxc.comments.annotation);
    }

    #[test]
    fn test_resolve_generator_stage_options_enables_comments_when_raw_disabled()
    {
        let resolved: ResolvedGeneratorStageOptions =
            resolve_generator_stage_options(Some(&GeneratorOptions {
                comments: Some(true),
                oxc: Some(PartialOxcCodegenOptions {
                    comments: Some(PartialOxcCommentOptions {
                        normal: Some(false),
                        jsdoc: Some(false),
                        annotation: Some(false),
                        legal: Some(oxc::codegen::LegalComment::None),
                    }),
                    ..PartialOxcCodegenOptions::default()
                }),
                ..GeneratorOptions::default()
            }));

        assert!(resolved.comments);
        assert!(resolved.oxc.comments.normal);
        assert!(resolved.oxc.comments.jsdoc);
        assert!(resolved.oxc.comments.annotation);
    }

    #[test]
    fn test_resolve_generator_stage_options_preserves_partial_raw_comments() {
        let resolved: ResolvedGeneratorStageOptions =
            resolve_generator_stage_options(Some(&GeneratorOptions {
                comments: Some(true),
                oxc: Some(PartialOxcCodegenOptions {
                    comments: Some(PartialOxcCommentOptions {
                        normal: Some(false),
                        jsdoc: Some(true),
                        annotation: Some(false),
                        legal: Some(oxc::codegen::LegalComment::None),
                    }),
                    ..PartialOxcCodegenOptions::default()
                }),
                ..GeneratorOptions::default()
            }));

        assert!(resolved.comments);
        assert!(!resolved.oxc.comments.normal);
        assert!(resolved.oxc.comments.jsdoc);
        assert!(!resolved.oxc.comments.annotation);
        assert_eq!(
            resolved.oxc.comments.legal,
            oxc::codegen::LegalComment::None
        );
    }

    #[test]
    fn test_resolve_options_forwards_raw_only_fields() {
        let resolved: ResolvedOptions = resolve_options(
            ResolvableOptions {
                parser: Some(ParserOptions {
                    oxc: Some(PartialOxcParseOptions {
                        enable_ident_hashes: Some(false),
                        ..PartialOxcParseOptions::default()
                    }),
                    ..ParserOptions::default()
                }),
                generator: Some(GeneratorOptions {
                    oxc: Some(PartialOxcCodegenOptions {
                        initial_indent: Some(1),
                        ..PartialOxcCodegenOptions::default()
                    }),
                    ..GeneratorOptions::default()
                }),
                ..ResolvableOptions::default()
            },
            vec![],
        );

        assert!(!resolved.parser.oxc.enable_ident_hashes);
        assert_eq!(resolved.generator.oxc.initial_indent, 1);
    }

    #[test]
    fn test_resolve_options_populates_parser_and_generator_stages() {
        let resolved: ResolvedOptions = resolve_options(
            ResolvableOptions {
                parser: Some(ParserOptions {
                    preserve_parens: Some(true),
                    ..ParserOptions::default()
                }),
                generator: Some(GeneratorOptions {
                    single_quote: Some(true),
                    indent_style: Some(IndentStyle::Tab),
                    ..GeneratorOptions::default()
                }),
                ..ResolvableOptions::default()
            },
            vec![],
        );

        assert!(resolved.parser.preserve_parens);
        assert!(resolved.parser.oxc.preserve_parens);
        assert!(resolved.generator.single_quote);
        assert_eq!(resolved.generator.indent_style, IndentStyle::Tab);
        assert_eq!(resolved.generator.oxc.indent_char, IndentChar::Tab);
    }

    #[test]
    fn test_resolve_options_without_groups_uses_telarel_defaults() {
        let resolved: ResolvedOptions =
            resolve_options(ResolvableOptions::default(), vec![]);

        assert!(!resolved.parser.preserve_parens);
        assert!(!resolved.parser.allow_return_outside_function);
        assert_eq!(resolved.generator.indent_style, IndentStyle::Space);
        assert_eq!(resolved.generator.indent_width, 2);
        assert!(resolved.generator.comments);
        assert!(!resolved.generator.minify);
    }
}
