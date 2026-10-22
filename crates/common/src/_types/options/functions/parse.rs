use std::path::PathBuf;

use crate::_types::grammar::language::Language;
use crate::_types::grammar::source_type::SourceType;
use crate::_types::options::source_map::input::InputSourceMap;

/// User options for a parse step; every field is optional and resolved
/// against defaults before use.
#[derive(Debug, Clone, Default)]
pub struct ParseOptions {
    /// Current working directory;
    /// `None` resolves to the process working directory.
    pub cwd: Option<PathBuf>,
    /// The file being parsed;
    /// `None` resolves to the default file name.
    pub file: Option<String>,
    /// The grammar of the code;
    /// `None` infers the grammar from the file extension.
    pub language: Option<Language>,
    /// The module system of the code;
    /// `None` keeps the module kind resolved from the grammar.
    pub source_type: Option<SourceType>,
    /// The input source-map request for the step;
    /// `None` means no input-map lookup.
    pub input_source_map: Option<InputSourceMap>,
}
