use std::path::PathBuf;

use crate::_types::grammar::language::Language;
use crate::_types::grammar::source_type::SourceType;
use crate::_types::options::output::OutputOptions;
use crate::_types::options::sourcemap::SourceMapOptions;

/// User options for compiling an existing AST.
///
/// Every field is optional. Each field resolves against defaults before use.
#[derive(Debug, Clone, Default)]
pub struct CompileFromAstOptions {
    /// Current working directory.
    ///
    /// `None` resolves to the process working directory.
    pub cwd: Option<PathBuf>,
    /// The file being compiled.
    ///
    /// `None` resolves to the default file name.
    pub file: Option<String>,
    /// The grammar of the code.
    ///
    /// `None` falls back to the input AST grammar, then the file extension.
    pub language: Option<Language>,
    /// The module kind of the code.
    ///
    /// `None` falls back to the input AST module kind, then the file extension.
    pub source_type: Option<SourceType>,
    /// The output code configuration.
    pub output: Option<OutputOptions>,
    /// The source-map inputs and outputs requested for the step.
    pub source_map: Option<SourceMapOptions>,
}
