use std::fmt;

use oxc::diagnostics::{Error, NamedSource, OxcDiagnostic};
use oxc::span::Span;

use crate::contexts::compile::CompileContext;

/// An error that occurred during a compile run.
///
/// Wraps an [`oxc::diagnostics::Error`]; `Display` renders either the
/// graphical source-bound report (file:line:col + underlined source) or the
/// bare message when no source is attached.
pub struct CompileError {
    error: Error,
    rendered: String,
}

impl CompileError {
    /// Create an error attached to a source span.
    pub fn new(
        context: &CompileContext<'_>,
        span: Span,
        message: String,
    ) -> Self {
        let diagnostic: OxcDiagnostic =
            OxcDiagnostic::error(message).with_label(span);

        let rendered: String = diagnostic.clone().render_with_source_code(
            NamedSource::new(context.file, context.code.to_string()),
        );

        let error: Error = diagnostic.with_source_code(NamedSource::new(
            context.file,
            context.code.to_string(),
        ));

        Self { error, rendered }
    }

    /// Create a source-less error from a bare message.
    pub fn from_message(message: &str) -> Self {
        let diagnostic: OxcDiagnostic =
            OxcDiagnostic::error(message.to_string());

        Self { error: Error::from(diagnostic), rendered: message.to_string() }
    }
}

impl fmt::Display for CompileError {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        f.write_str(&self.rendered)
    }
}

impl fmt::Debug for CompileError {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        f.debug_struct("CompileError").field("error", &self.error).finish()
    }
}

impl std::error::Error for CompileError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_span_error_displays_file_and_message() {
        let cwd: &str = "/repo";
        let file: &str = "src/app.js";
        let code: &str = "let 1 = x;";
        let ctx: CompileContext<'_> = CompileContext::new(cwd, file, code);
        let span: oxc::span::Span = oxc::span::Span::new(4, 5);
        let error: CompileError =
            CompileError::new(&ctx, span, "expected an identifier".into());
        let display: String = error.to_string();

        assert!(display.contains("src/app.js"));
        assert!(display.contains("expected an identifier"));
    }

    #[test]
    fn test_from_message_displays_message() {
        let error: CompileError = CompileError::from_message("boom");
        assert_eq!(error.to_string(), "boom");
    }

    #[test]
    fn test_compile_error_is_std_error() {
        let error: CompileError = CompileError::from_message("boom");
        let boxed: Box<dyn std::error::Error> = Box::new(error);

        assert_eq!(boxed.to_string(), "boom");
    }

    #[test]
    fn test_debug_renders() {
        let cwd: &str = "/repo";
        let file: &str = "src/app.js";
        let code: &str = "let 1 = x;";
        let ctx: CompileContext<'_> = CompileContext::new(cwd, file, code);
        let span: oxc::span::Span = oxc::span::Span::new(4, 5);
        let error: CompileError =
            CompileError::new(&ctx, span, "expected an identifier".into());
        let debug: String = format!("{:?}", error);

        assert!(debug.contains("CompileError"));
    }
}
