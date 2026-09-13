use oxc::allocator::Allocator;
use oxc::ast::ast::Program;
use oxc::diagnostics::LabeledSpan;
use oxc::parser::Parser;
use oxc::span::{SourceType, Span};

use crate::contexts::compile::CompileContext;
use crate::errors::compile::CompileError;

/// Options for [`parse`].
#[derive(Clone, Copy)]
pub struct ParseOptions<'ctx, 'a> {
    /// Compile context for diagnostics.
    pub context: &'ctx CompileContext<'ctx>,
    /// Destination allocator for the parsed program.
    pub allocator: &'a Allocator,
    /// File path.
    pub file: &'a str,
    /// Source code to parse.
    pub code: &'a str,
}

/// Result of [`parse`].
#[derive(Debug)]
pub struct ParseResult<'a> {
    /// The parsed program.
    pub program: Program<'a>,
}

/// Drop the `?query`/`#hash` suffix from a module id.
fn strip_id_suffix(module_id: &str) -> &str {
    let without_hash: &str =
        module_id.split_once('#').map_or(module_id, |(path, _)| path);
    without_hash.split_once('?').map_or(without_hash, |(path, _)| path)
}

/// Infer the source type from a module id, falling back to the default
/// (an ES module) for extensionless or unknown virtual files.
fn source_type_for_module_id(module_id: &str) -> SourceType {
    let path: &str = strip_id_suffix(module_id);
    SourceType::from_path(path).unwrap_or_default()
}

/// Parse source code into a [`Program`].
pub fn parse<'ctx, 'a>(
    options: ParseOptions<'ctx, 'a>
) -> Result<ParseResult<'a>, CompileError> {
    let source_type: SourceType = source_type_for_module_id(options.file);

    let parser_return: oxc::parser::ParserReturn<'_> =
        Parser::new(options.allocator, options.code, source_type).parse();

    if let Some(diagnostic) = parser_return.diagnostics.first() {
        let span: Span = diagnostic
            .labels
            .first()
            .map(|label: &LabeledSpan| {
                Span::new(label.offset(), label.offset() + label.len())
            })
            .unwrap_or_else(|| Span::new(0, 0));

        let message: String = diagnostic.to_string();

        return Err(CompileError::new(options.context, span, message));
    }

    if parser_return.fatal_error {
        let span: Span = Span::new(0, 0);
        return Err(CompileError::new(
            options.context,
            span,
            "the parser aborted".to_string(),
        ));
    }

    Ok(ParseResult { program: parser_return.program })
}

#[cfg(test)]
mod tests {
    use oxc::allocator::Allocator;

    use super::*;

    const CWD: &str = "";

    const FILE: &str = "index.ts";

    fn parse_code<'a>(
        allocator: &'a Allocator,
        file: &'a str,
        code: &'a str,
    ) -> Result<ParseResult<'a>, CompileError> {
        let context: CompileContext<'_> = CompileContext::new(CWD, file, code);

        let options: ParseOptions<'_, 'a> =
            ParseOptions { context: &context, allocator, file, code };

        parse(options)
    }

    #[test]
    fn test_parse_ok() {
        let allocator: Allocator = Allocator::default();
        let result: ParseResult<'_> =
            parse_code(&allocator, FILE, "const a = 1;")
                .expect("valid source parses");

        assert!(!result.program.body.is_empty());
        assert!(result.program.source_type.is_typescript());
    }

    #[test]
    fn test_parse_invalid_syntax_errors() {
        let allocator: Allocator = Allocator::default();
        let result: Result<ParseResult<'_>, CompileError> =
            parse_code(&allocator, FILE, "const = ;");

        assert!(result.is_err());
    }

    #[test]
    fn test_parse_strips_query_suffix() {
        let allocator: Allocator = Allocator::default();
        let file: &str = "index.js?v=d48eb52a";
        let result: ParseResult<'_> =
            parse_code(&allocator, file, "const x = 1;")
                .expect("query suffix is stripped");

        assert!(result.program.source_type.is_javascript());
        assert!(!result.program.source_type.is_typescript());
    }

    #[test]
    fn test_parse_strips_hash_suffix() {
        let allocator: Allocator = Allocator::default();
        let file: &str = "index.ts#hash";
        let result: ParseResult<'_> =
            parse_code(&allocator, file, "const x = 1;")
                .expect("hash suffix is stripped");

        assert!(result.program.source_type.is_typescript());
    }

    #[test]
    fn test_parse_strips_query_and_hash() {
        let allocator: Allocator = Allocator::default();
        let file: &str = "index.tsx?v=1#h";
        let result: ParseResult<'_> =
            parse_code(&allocator, file, "const x = 1;")
                .expect("query and hash suffixes are stripped");

        assert!(result.program.source_type.is_typescript());
    }

    #[test]
    fn test_parse_strips_hash_before_query() {
        let allocator: Allocator = Allocator::default();
        let file: &str = "index.ts#h?v=1";
        let result: ParseResult<'_> =
            parse_code(&allocator, file, "const x = 1;")
                .expect("hash-before-query suffix is stripped");

        assert!(result.program.source_type.is_typescript());
    }

    #[test]
    fn test_parse_unknown_extension_falls_back_to_esm() {
        let allocator: Allocator = Allocator::default();
        let file: &str = "index.unknown";
        let result: Result<ParseResult<'_>, CompileError> =
            parse_code(&allocator, file, "const x = 1;");

        assert!(result.is_ok());

        let parsed: ParseResult<'_> = result.expect("ESM fallback parses");

        assert!(parsed.program.source_type.is_javascript());
        assert!(!parsed.program.source_type.is_typescript());
    }
}
