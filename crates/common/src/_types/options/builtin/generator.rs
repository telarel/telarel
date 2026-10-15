use crate::_types::options::source_map::SourceMapOptions;

/// Resolved options for the builtin [`crate::ast::generator::Generator`].
#[derive(Debug, Clone)]
pub struct GeneratorOptions {
    /// Current working directory (reserved; builtin codegen does not use it).
    pub cwd: String,
    /// The file being generated.
    pub file: String,
    /// The source-map inputs and outputs requested for the step.
    pub source_map: SourceMapOptions,
}
