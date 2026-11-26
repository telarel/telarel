use std::sync::Arc;

use oxc::span::Span;

use crate::ast::ast::Ast;
use crate::errors::compile::CompileError;
use crate::errors::plugin::PluginError;
use crate::errors::source::ErrorSource;

impl ErrorSource {
    /// Create a source location that shares the AST's source text.
    pub fn from_ast(
        file: impl Into<Arc<str>>,
        ast: &Ast,
        span: Span,
    ) -> Self {
        Self::from_code(file, Arc::clone(ast.source()), span)
    }
}

impl CompileError {
    /// Attach a source location that shares the AST's source text.
    pub fn with_source_ast(
        self,
        file: impl Into<Arc<str>>,
        ast: &Ast,
        span: Span,
    ) -> Self {
        self.with_source(ErrorSource::from_ast(file, ast, span))
    }
}

impl PluginError {
    /// Attach a source location that shares the AST's source text.
    pub fn with_source_ast(
        self,
        file: impl Into<Arc<str>>,
        ast: &Ast,
        span: Span,
    ) -> Self {
        self.with_source(ErrorSource::from_ast(file, ast, span))
    }
}
