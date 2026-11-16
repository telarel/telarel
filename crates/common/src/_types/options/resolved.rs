use std::path::PathBuf;

use crate::_types::grammar::language::Language;
use crate::_types::grammar::source_type::SourceType;
use crate::_types::options::builtin::generator::ResolvedGeneratorStageOptions;
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

#[cfg(test)]
mod tests {
    use crate::_types::grammar::language::Language;
    use crate::_types::grammar::source_type::SourceType;
    use crate::_types::options::builtin::generator::IndentStyle;

    use super::*;

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
}
