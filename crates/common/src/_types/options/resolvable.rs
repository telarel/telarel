use std::path::PathBuf;

use crate::_types::grammar::language::Language;
use crate::_types::grammar::source_type::SourceType;
use crate::_types::options::functions::compile_from_ast::CompileFromAstOptions;
use crate::_types::options::functions::generate::GenerateOptions;
use crate::_types::options::functions::parse::ParseOptions;
use crate::_types::options::functions::rewrite::RewriteOptions;
use crate::_types::options::output::OutputOptions;
use crate::_types::options::sourcemap::SourceMapOptions;
use crate::_types::options::sourcemap::input::InputSourceMap;

/// The union of every step's optional settings, resolved once per compile
/// run; every field is optional.
#[derive(Debug, Clone, Default)]
pub struct ResolvableOptions {
    /// Current working directory;
    /// `None` resolves to the process working directory.
    pub cwd: Option<PathBuf>,
    /// The file being compiled;
    /// `None` resolves to the default file name.
    pub file: Option<String>,
    /// The grammar of the code;
    /// `None` infers the grammar from the file extension.
    pub language: Option<Language>,
    /// The module system of the code;
    /// `None` keeps the module kind resolved from the grammar.
    pub source_type: Option<SourceType>,
    /// The output code settings.
    pub output: Option<OutputOptions>,
    /// The source-map inputs and outputs requested for the run.
    pub source_map: Option<SourceMapOptions>,
}

/// Normalize a flat input request into the shared source-map bag.
fn wrap_input_source_map(
    input: Option<InputSourceMap>
) -> Option<SourceMapOptions> {
    input.map(|input: InputSourceMap| SourceMapOptions {
        input: Some(input),
        output: None,
    })
}

impl From<ParseOptions> for ResolvableOptions {
    fn from(options: ParseOptions) -> Self {
        Self {
            cwd: options.cwd,
            file: options.file,
            language: options.language,
            source_type: options.source_type,
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
            language: None,
            source_type: None,
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
    fn test_from_parse_options_wraps_input_source_map() {
        let options: ParseOptions = ParseOptions {
            cwd: Some(PathBuf::from("/repo")),
            file: Some(String::from("src/app.ts")),
            language: Some(Language::TS),
            source_type: Some(SourceType::Module),
            input_source_map: Some(InputSourceMap::Mode(
                InputSourceMapMode::Auto,
            )),
        };

        let resolvable: ResolvableOptions = ResolvableOptions::from(options);

        assert_eq!(resolvable.cwd, Some(PathBuf::from("/repo")));
        assert_eq!(resolvable.file.as_deref(), Some("src/app.ts"));
        assert_eq!(resolvable.language, Some(Language::TS));
        assert_eq!(resolvable.source_type, Some(SourceType::Module));

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

        let source_map: SourceMapOptions =
            resolvable.source_map.expect("input map is carried");

        assert!(matches!(
            source_map.input,
            Some(InputSourceMap::Mode(InputSourceMapMode::Inline)),
        ));
        assert!(source_map.output.is_none());
    }

    #[test]
    fn test_from_generate_options_drops_language_and_source_type() {
        let options: GenerateOptions = GenerateOptions {
            cwd: Some(PathBuf::from("/repo")),
            file: Some(String::from("src/app.js")),
            output: Some(OutputOptions {
                file: Some(String::from("dist/app.js")),
            }),
            source_map: Some(SourceMapOptions::default()),
        };

        let resolvable: ResolvableOptions = ResolvableOptions::from(options);

        assert_eq!(resolvable.cwd, Some(PathBuf::from("/repo")));
        assert_eq!(resolvable.file.as_deref(), Some("src/app.js"));
        assert_eq!(resolvable.language, None);
        assert_eq!(resolvable.source_type, None);
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
