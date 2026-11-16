use std::path::PathBuf;

use crate::_types::grammar::language::Language;
use crate::_types::grammar::source_type::SourceType;
use crate::_types::options::builtin::parser::ParserOptions;
use crate::_types::options::sourcemap::input::InputSourceMap;

/// User options for a parse step.
///
/// Every field is optional. Each field resolves against defaults before use.
#[derive(Debug, Clone, Default)]
pub struct ParseOptions {
    /// Current working directory.
    ///
    /// By default, it is the process working directory.
    pub cwd: Option<PathBuf>,
    /// The file being parsed.
    ///
    /// By default, it is `"index.ts"`.
    pub file: Option<String>,
    /// The grammar of the code.
    ///
    /// By default, it is inferred from the file extension.
    pub language: Option<Language>,
    /// The module system of the code.
    ///
    /// By default, it is resolved from the grammar.
    pub source_type: Option<SourceType>,
    /// Parser-stage options.
    ///
    /// By default, it is `None`, which applies the `ParserOptions` defaults.
    pub parser: Option<ParserOptions>,
    /// The input source-map request for the step.
    ///
    /// By default, it is `None`, which disables input-map lookup.
    pub input_source_map: Option<InputSourceMap>,
}
