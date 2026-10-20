use std::fmt;
use std::sync::Arc;

use oxc::span::Span;

use crate::ast::ast::Ast;

/// A source location attached to an error.
///
/// Shares the source text, so attaching it to many errors does not copy the
/// code.
#[derive(Clone, PartialEq, Eq)]
pub struct ErrorSource {
    file: Arc<str>,
    code: Arc<str>,
    span: Span,
}

impl ErrorSource {
    /// Create a source location from owned or shared code.
    pub fn from_code(
        file: impl Into<Arc<str>>,
        code: impl Into<Arc<str>>,
        span: Span,
    ) -> Self {
        Self { file: file.into(), code: code.into(), span }
    }

    /// Create a source location that shares the AST's source text.
    pub fn from_ast(
        file: impl Into<Arc<str>>,
        ast: &Ast,
        span: Span,
    ) -> Self {
        Self::from_code(file, Arc::clone(ast.source()), span)
    }

    /// The path of the file the span belongs to.
    pub fn file(&self) -> &str {
        &self.file
    }

    /// The source text the span refers to.
    pub fn code(&self) -> &str {
        &self.code
    }

    /// The byte-offset span within [`ErrorSource::code`].
    pub fn span(&self) -> Span {
        self.span
    }
}

impl fmt::Debug for ErrorSource {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        f.debug_struct("ErrorSource")
            .field("file", &self.file)
            .field("span", &self.span)
            .field("code_len", &self.code.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::parse::{ParseAstOptions, parse_ast};

    fn parse(code: &str) -> Ast {
        let options: ParseAstOptions = ParseAstOptions {
            cwd: String::new(),
            file: "index.ts".to_string(),
            language: None,
            source_type: None,
            input_map: None,
        };

        parse_ast(code.to_string(), options).expect("valid source parses")
    }

    #[test]
    fn test_from_code_exposes_parts() {
        let span: Span = Span::new(4, 5);
        let source: ErrorSource =
            ErrorSource::from_code("src/app.js", "let 1 = x;", span);

        assert_eq!(source.file(), "src/app.js");
        assert_eq!(source.code(), "let 1 = x;");
        assert_eq!(source.span(), span);
    }

    #[test]
    fn test_from_ast_uses_ast_source() {
        let ast: Ast = parse("const a = 1;");
        let span: Span = Span::new(6, 7);
        let source: ErrorSource = ErrorSource::from_ast("index.ts", &ast, span);

        assert_eq!(source.file(), "index.ts");
        assert_eq!(source.code(), ast.source().as_ref());
        assert_eq!(source.span(), span);
    }

    #[test]
    fn test_debug_does_not_dump_source_text() {
        let span: Span = Span::new(0, 1);
        let source: ErrorSource = ErrorSource::from_code(
            "src/app.js",
            "let secret_identifier = 1;",
            span,
        );
        let debug: String = format!("{:?}", source);

        assert!(debug.contains("src/app.js"));
        assert!(!debug.contains("secret_identifier"));
    }
}
