use telarel_common::ResolvedOptions;

/// Arguments for the `start` hook.
#[derive(Debug, Clone)]
pub struct StartArgs {
    /// Resolved options, read-only.
    pub options: ResolvedOptions,
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use telarel_common::{
        IndentStyle, Language, ResolvedGeneratorStageOptions, ResolvedOptions,
        ResolvedOutputOptions, ResolvedParserStageOptions, SourceMapOptions,
        SourceType,
    };

    use super::*;

    #[test]
    fn test_start_args_carry_resolved_options() {
        let args: StartArgs = StartArgs {
            options: ResolvedOptions {
                cwd: PathBuf::from("/repo"),
                file: String::from("a.ts"),
                language: Language::TS,
                source_type: SourceType::Module,
                parser: ResolvedParserStageOptions {
                    preserve_parens: true,
                    allow_return_outside_function: true,
                    ..ResolvedParserStageOptions::default()
                },
                generator: ResolvedGeneratorStageOptions {
                    single_quote: true,
                    ascii_only: true,
                    indent_style: IndentStyle::Tab,
                    indent_width: 4,
                    comments: false,
                    minify: true,
                    ..ResolvedGeneratorStageOptions::default()
                },
                output: ResolvedOutputOptions {
                    file: String::from("dist/a.js"),
                },
                source_map: SourceMapOptions::default(),
                plugins: vec![],
            },
        };

        assert_eq!(args.options.cwd, PathBuf::from("/repo"));
        assert_eq!(args.options.plugins.len(), 0);
        assert!(args.options.parser.preserve_parens);
        assert!(args.options.parser.allow_return_outside_function);
        assert!(args.options.generator.single_quote);
        assert!(args.options.generator.ascii_only);
        assert_eq!(args.options.generator.indent_style, IndentStyle::Tab);
        assert_eq!(args.options.generator.indent_width, 4);
        assert!(!args.options.generator.comments);
        assert!(args.options.generator.minify);
    }
}
