use std::path::PathBuf;

use crate::_types::grammar::language::Language;
use crate::_types::grammar::source_type::SourceType;
use crate::_types::options::output::OutputOptions;
use crate::_types::options::sourcemap::SourceMapOptions;

/// User options for compiling from an existing AST; every field is optional
/// and resolved against defaults before use.
#[derive(Debug, Clone, Default)]
pub struct CompileFromAstOptions {
    /// Current working directory;
    /// `None` resolves to the process working directory.
    pub cwd: Option<PathBuf>,
    /// The file being compiled;
    /// `None` resolves to the default file name.
    pub file: Option<String>,
    /// The grammar of the code;
    /// `None` falls back to the input AST grammar, then the file extension.
    pub language: Option<Language>,
    /// The module system of the code;
    /// `None` keeps the module kind resolved from the grammar.
    pub source_type: Option<SourceType>,
    /// The output code settings.
    pub output: Option<OutputOptions>,
    /// The source-map inputs and outputs requested for the step.
    pub source_map: Option<SourceMapOptions>,
}
