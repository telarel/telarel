use std::path::PathBuf;

use crate::_types::grammar::language::Language;
use crate::_types::grammar::source_type::SourceType;
use crate::_types::options::sourcemap::input::InputSourceMap;

/// User options for a rewrite step.
///
/// Every field is optional. Each field resolves against defaults before use.
#[derive(Debug, Clone, Default)]
pub struct RewriteOptions {
    /// Current working directory.
    ///
    /// `None` resolves to the process working directory.
    pub cwd: Option<PathBuf>,
    /// The file being rewritten.
    ///
    /// `None` resolves to the default file name.
    pub file: Option<String>,
    /// The grammar of the code.
    ///
    /// `None` falls back to the input AST grammar, then the file extension.
    pub language: Option<Language>,
    /// The module kind of the code. A `source_type` other than `Unambiguous`
    /// replaces the module kind of the input AST. `Unambiguous` requests
    /// inference, so the AST module kind is the inferred result. If the AST has
    /// no module kind, the file extension is the fallback.
    pub source_type: Option<SourceType>,
    /// The input source-map request for the step.
    ///
    /// `None` means no input-map lookup.
    pub input_source_map: Option<InputSourceMap>,
}
