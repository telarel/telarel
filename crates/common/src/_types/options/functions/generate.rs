use std::path::PathBuf;

use crate::_types::grammar::language::Language;
use crate::_types::options::output::OutputOptions;
use crate::_types::options::sourcemap::SourceMapOptions;

/// User options for a generate step; every field is optional and resolved
/// against defaults before use.
#[derive(Debug, Clone, Default)]
pub struct GenerateOptions {
    /// Current working directory;
    /// `None` resolves to the process working directory.
    pub cwd: Option<PathBuf>,
    /// The file being generated;
    /// `None` resolves to the default file name.
    pub file: Option<String>,
    /// The grammar used for resolution and visible to plugins; `None` falls
    /// back to the AST grammar, then the file extension for AST-entry stages.
    pub language: Option<Language>,
    /// The output code settings.
    pub output: Option<OutputOptions>,
    /// The source-map inputs and outputs requested for the step.
    pub source_map: Option<SourceMapOptions>,
}
