use std::path::PathBuf;

use crate::_types::grammar::language::Language;
use crate::_types::options::builtin::generator::GeneratorOptions;
use crate::_types::options::output::OutputOptions;
use crate::_types::options::sourcemap::SourceMapOptions;

/// User options for a generate step.
///
/// Every field is optional. Each field resolves against defaults before use.
#[derive(Debug, Clone, Default)]
pub struct GenerateOptions {
    /// Current working directory.
    ///
    /// By default, it is the process working directory.
    pub cwd: Option<PathBuf>,
    /// The file being generated.
    ///
    /// By default, it is `"index.ts"`.
    pub file: Option<String>,
    /// The grammar used for resolution and visible to plugins.
    ///
    /// By default, it is taken from the AST grammar, then inferred from the
    /// file extension.
    pub language: Option<Language>,
    /// Generator-stage options.
    ///
    /// By default, it is `None`, which applies the `GeneratorOptions` defaults.
    pub generator: Option<GeneratorOptions>,
    /// The output code configuration.
    ///
    /// By default, it is `None`.
    pub output: Option<OutputOptions>,
    /// The source-map inputs and outputs requested for the step.
    ///
    /// By default, it is `None`, which disables source-map input lookup and
    /// output delivery.
    pub source_map: Option<SourceMapOptions>,
}
