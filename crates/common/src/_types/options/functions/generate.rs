use std::path::PathBuf;

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
    /// The output code settings.
    pub output: Option<OutputOptions>,
    /// The source-map inputs and outputs requested for the step.
    pub source_map: Option<SourceMapOptions>,
}
