use crate::_types::grammar::language::Language;
use crate::_types::grammar::source_type::SourceType;
use crate::_types::options::source_map::SourceMapOptions;

/// User options for compiling from an existing AST; every field is optional
/// and resolved against defaults before use.
#[derive(Debug, Clone, Default)]
pub struct CompileFromAstOptions {
    /// Current working directory;
    /// `None` resolves to the process working directory.
    pub cwd: Option<String>,
    /// The file being compiled;
    /// `None` resolves to a default filename.
    pub file: Option<String>,
    /// The grammar of the code;
    /// `None` infers the grammar from the file extension.
    pub language: Option<Language>,
    /// The module system of the code;
    /// `None` keeps the module kind resolved from the grammar.
    pub source_type: Option<SourceType>,
    /// The source-map inputs and outputs requested for the step.
    pub source_map: Option<SourceMapOptions>,
}
