use std::fmt;
use std::sync::Arc;

use oxc::span::Span;

use crate::ast::ast::Ast;
use crate::errors::compile::{CompileError, render_source};
use crate::errors::source::ErrorSource;

/// An error produced by a plugin hook.
///
/// Carries a message and an optional source location. `Display` renders the
/// message alone, or a graphical code frame when a source is attached.
#[derive(Clone, PartialEq, Eq)]
pub struct PluginError {
    message: String,
    source: Option<ErrorSource>,
}

impl PluginError {
    /// Create a source-less error from a message.
    pub fn new(message: impl Into<String>) -> Self {
        Self { message: message.into(), source: None }
    }

    /// Attach a source location to this error.
    pub fn with_source(
        self,
        source: ErrorSource,
    ) -> Self {
        Self { source: Some(source), ..self }
    }

    /// Attach a source location from file text and a byte span.
    pub fn with_source_code(
        self,
        file: impl Into<Arc<str>>,
        code: impl Into<Arc<str>>,
        span: Span,
    ) -> Self {
        self.with_source(ErrorSource::from_code(file, code, span))
    }

    /// Attach a source location that shares the AST's source text.
    pub fn with_source_ast(
        self,
        file: impl Into<Arc<str>>,
        ast: &Ast,
        span: Span,
    ) -> Self {
        self.with_source(ErrorSource::from_ast(file, ast, span))
    }

    /// The error message.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// The attached source location, if any.
    pub fn source(&self) -> Option<&ErrorSource> {
        self.source.as_ref()
    }

    /// Convert into a [`CompileError`], moving the message and source.
    pub fn into_compile_error(self) -> CompileError {
        let error: CompileError = CompileError::new(self.message);

        match self.source {
            | Some(source) => error.with_source(source),
            | None => error,
        }
    }
}

impl fmt::Display for PluginError {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        match &self.source {
            | Some(source) => {
                f.write_str(&render_source(&self.message, source))
            },
            | None => f.write_str(&self.message),
        }
    }
}

impl fmt::Debug for PluginError {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        let mut debug = f.debug_struct("PluginError");

        debug.field("message", &self.message());

        if let Some(source) = self.source() {
            debug.field("file", &source.file());
            debug.field("span", &source.span());
        }

        debug.finish()
    }
}

impl std::error::Error for PluginError {}

#[cfg(test)]
mod tests {
    use oxc::span::Span;

    use super::*;

    #[test]
    fn test_source_less_display() {
        let error: PluginError = PluginError::new("boom");

        assert_eq!(error.to_string(), "boom");
        assert!(error.source().is_none());
    }

    #[test]
    fn test_source_bound_display_contains_file_and_message() {
        let span: Span = Span::new(4, 5);

        let error: PluginError = PluginError::new("expected an identifier")
            .with_source_code("src/app.js", "let 1 = x;", span);

        let display: String = error.to_string();

        assert!(display.contains("src/app.js"));
        assert!(display.contains("expected an identifier"));
    }

    #[test]
    fn test_into_compile_error_preserves_message_and_source() {
        let span: Span = Span::new(4, 5);

        let error: PluginError = PluginError::new("expected an identifier")
            .with_source_code("src/app.js", "let 1 = x;", span);

        let expected_source: Option<ErrorSource> = error.source().cloned();

        let compile_error: CompileError = error.into_compile_error();

        assert_eq!(compile_error.message(), "expected an identifier");
        assert_eq!(compile_error.source(), expected_source.as_ref());
    }

    #[test]
    fn test_into_compile_error_without_source() {
        let error: PluginError = PluginError::new("boom");

        let compile_error: CompileError = error.into_compile_error();

        assert_eq!(compile_error.message(), "boom");
        assert_eq!(compile_error.to_string(), "boom");
        assert!(compile_error.source().is_none());
    }

    #[test]
    fn test_debug_does_not_dump_source_text() {
        let span: Span = Span::new(0, 1);

        let error: PluginError = PluginError::new("boom").with_source_code(
            "src/app.js",
            "let secret_identifier = 1;",
            span,
        );

        let debug: String = format!("{:?}", error);

        assert!(debug.contains("PluginError"));
        assert!(debug.contains("boom"));
        assert!(debug.contains("src/app.js"));
        assert!(!debug.contains("secret_identifier"));
    }
}
