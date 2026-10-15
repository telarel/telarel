use crate::_types::grammar::language::Language;
use crate::_types::grammar::source_type::SourceType;
use crate::_types::options::source_map::SourceMapOptions;

/// Resolved options for the builtin [`crate::ast::parser::Parser`].
#[derive(Debug, Clone)]
pub struct ParserOptions {
    /// Current working directory.
    pub cwd: String,
    /// The file being parsed.
    pub file: String,
    /// The grammar of the code.
    pub language: Language,
    /// The module system of the code.
    pub source_type: SourceType,
    /// The source-map inputs and outputs requested for the step.
    pub source_map: SourceMapOptions,
}
