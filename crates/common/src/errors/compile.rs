use std::fmt;
use std::sync::Arc;

use oxc::diagnostics::{NamedSource, OxcDiagnostic};
use oxc::span::Span;

use crate::ast::ast::Ast;
use crate::errors::source::ErrorSource;

pub fn render_source(
    message: &str,
    source: &ErrorSource,
) -> String {
    OxcDiagnostic::error(message.to_string())
        .with_label(source.span())
        .render_with_source_code(NamedSource::new(
            source.file(),
            source.code().to_string(),
        ))
}

/// An error produced during a compile run.
///
/// Carries a message and an optional source location. `Display` renders the
/// message alone, or a graphical code frame when a source is attached.
#[derive(Clone, PartialEq, Eq)]
pub struct CompileError {
    message: String,
    source: Option<ErrorSource>,
}

impl CompileError {
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
}

impl fmt::Display for CompileError {
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

impl fmt::Debug for CompileError {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        let mut debug = f.debug_struct("CompileError");

        debug.field("message", &self.message);

        if let Some(source) = &self.source {
            debug.field("file", &source.file());
            debug.field("span", &source.span());
        }

        debug.finish()
    }
}

impl std::error::Error for CompileError {}

#[cfg(test)]
mod tests {
    use oxc::span::Span;

    use super::*;

    #[test]
    fn test_span_error_displays_file_and_message() {
        let span: Span = Span::new(4, 5);

        let error: CompileError = CompileError::new("expected an identifier")
            .with_source_code("src/app.js", "let 1 = x;", span);

        let display: String = error.to_string();

        assert!(display.contains("src/app.js"));
        assert!(display.contains("expected an identifier"));
        assert!(display.contains('^'));
    }

    #[test]
    fn test_new_displays_message_without_source() {
        let error: CompileError = CompileError::new("boom");

        assert_eq!(error.to_string(), "boom");
        assert!(error.source().is_none());
    }

    #[test]
    fn test_compile_error_is_std_error() {
        let error: CompileError = CompileError::new("boom");

        let boxed: Box<dyn std::error::Error> = Box::new(error);

        assert_eq!(boxed.to_string(), "boom");
    }

    #[test]
    fn test_source_accessors_preserve_parts() {
        let span: Span = Span::new(4, 5);

        let error: CompileError = CompileError::new("expected an identifier")
            .with_source_code("src/app.js", "let 1 = x;", span);

        let source: &ErrorSource = error.source().expect("source attached");

        assert_eq!(error.message(), "expected an identifier");
        assert_eq!(source.file(), "src/app.js");
        assert_eq!(source.code(), "let 1 = x;");
        assert_eq!(source.span(), span);
    }

    #[test]
    fn test_source_ast_shares_ast_source() {
        use crate::ast::parse::{ParseAstOptions, parse_ast};

        let options: ParseAstOptions = ParseAstOptions {
            cwd: String::new(),
            file: "index.ts".to_string(),
            language: None,
            source_type: None,
            input_map: None,
        };

        let ast: Ast = parse_ast("const a = 1;".to_string(), options)
            .expect("valid source parses");

        let span: Span = Span::new(6, 7);

        let error: CompileError =
            CompileError::new("boom").with_source_ast("index.ts", &ast, span);

        assert_eq!(
            error.source().expect("source attached").code(),
            ast.source().as_ref()
        );
    }

    #[test]
    fn test_clone_and_eq() {
        let span: Span = Span::new(4, 5);

        let error: CompileError = CompileError::new("boom").with_source_code(
            "src/app.js",
            "let 1 = x;",
            span,
        );

        let clone: CompileError = error.clone();

        assert_eq!(clone, error);
        assert_ne!(CompileError::new("other"), error);
    }

    #[test]
    fn test_debug_does_not_dump_source_text() {
        let span: Span = Span::new(0, 1);

        let error: CompileError = CompileError::new("boom").with_source_code(
            "src/app.js",
            "let secret_identifier = 1;",
            span,
        );

        let debug: String = format!("{:?}", error);

        assert!(debug.contains("CompileError"));
        assert!(debug.contains("boom"));
        assert!(debug.contains("src/app.js"));
        assert!(debug.contains("start: 0"));
        assert!(debug.contains("end: 1"));
        assert!(!debug.contains("secret_identifier"));
    }
}
