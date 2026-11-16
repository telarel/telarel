use std::path::PathBuf;

use crate::_types::grammar::language::Language;
use crate::_types::grammar::source_type::SourceType;
use crate::_types::options::builtin::generator::GeneratorOptions;
use crate::_types::options::output::OutputOptions;
use crate::_types::options::sourcemap::SourceMapOptions;

/// User options for compiling an existing AST.
///
/// Every field is optional. Each field resolves against defaults before use.
#[derive(Debug, Clone, Default)]
pub struct CompileFromAstOptions {
    /// Current working directory.
    ///
    /// By default, it is the process working directory.
    pub cwd: Option<PathBuf>,
    /// The file being compiled.
    ///
    /// By default, it is `"index.ts"`.
    pub file: Option<String>,
    /// The grammar of the code.
    ///
    /// By default, it is taken from the input AST grammar, then inferred
    /// from the file extension.
    pub language: Option<Language>,
    /// The module kind of the code.
    ///
    /// By default, it is taken from the input AST module kind, then resolved
    /// from the file extension.
    pub source_type: Option<SourceType>,
    /// Generator-stage options.
    ///
    /// By default, it is `None`, which applies the `GeneratorOptions` defaults.
    pub generator: Option<GeneratorOptions>,
    /// The output code configuration.
    ///
    /// By default, it is `None`.
    pub output: Option<OutputOptions>,
    /// The source-map inputs and outputs requested for the step.
    ///
    /// By default, it is `None`, which disables source-map input lookup and
    /// output delivery.
    pub source_map: Option<SourceMapOptions>,
}
