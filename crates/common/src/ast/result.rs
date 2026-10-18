use crate::ast::ast::Ast;
use crate::sourcemap::SourceMap;

/// Result of a successful parse: the owned AST.
#[derive(Debug, Clone)]
pub struct AstResult {
    /// Parsed AST owning its allocator and source.
    pub ast: Ast,
}

/// Result of code generation: the generated code and its source map.
#[derive(Debug, Clone)]
pub struct CodeResult {
    /// Generated source code text.
    pub code: String,
    /// The source map, when one is produced.
    pub map: Option<SourceMap>,
}
