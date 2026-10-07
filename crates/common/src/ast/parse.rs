use std::sync::Arc;

use oxc::allocator::Allocator;
use oxc::ast::ast::Program;
use oxc::diagnostics::LabeledSpan;
use oxc::parser::Parser;
use oxc::span::{SourceType as OxcSourceType, Span};

use crate::_types::options::language::{Language, grammar_source_type};
use crate::_types::options::source_type::{SourceType, with_module_kind};
use crate::ast::ast::Ast;
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
    /// The grammar of the code;
    /// `None` infers the grammar from the file extension.
    pub language: Option<Language>,
    /// The module system of the code;
    /// `None` keeps the module kind resolved from the grammar.
    pub source_type: Option<SourceType>,
}

/// Options for [`parse_owned`].
#[derive(Clone, Copy)]
pub struct ParseOwnedOptions<'ctx, 'a> {
    /// Compile context for diagnostics.
    pub context: &'ctx CompileContext<'ctx>,
    /// File path.
    pub file: &'a str,
    /// Source code to parse.
    pub code: &'a str,
    /// The grammar of the code;
    /// `None` infers the grammar from the file extension.
    pub language: Option<Language>,
    /// The module system of the code;
    /// `None` keeps the module kind resolved from the grammar.
    pub source_type: Option<SourceType>,
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
pub fn source_type_for_module_id(module_id: &str) -> OxcSourceType {
    let path: &str = strip_id_suffix(module_id);
    OxcSourceType::from_path(path).unwrap_or_default()
}

/// Resolve the oxc source type: an explicit language wins, the module kind is
/// overlaid, and otherwise the module id's extension is inferred.
fn resolve_source_type(
    file: &str,
    language: Option<Language>,
    source_type: Option<SourceType>,
) -> OxcSourceType {
    let grammar: OxcSourceType = language
        .map(grammar_source_type)
        .unwrap_or_else(|| source_type_for_module_id(file));

    match source_type {
        | Some(module_kind) => with_module_kind(grammar, module_kind),
        | None => grammar,
    }
}

/// Convert the first parser diagnostic (or fatal error) into a
/// [`CompileError`].
fn parse_error(
    context: &CompileContext<'_>,
    parser_return: &oxc::parser::ParserReturn<'_>,
) -> Option<CompileError> {
    if let Some(diagnostic) = parser_return.diagnostics.first() {
        let span: Span = diagnostic
            .labels
            .first()
            .map(|label: &LabeledSpan| {
                Span::new(label.offset(), label.offset() + label.len())
            })
            .unwrap_or_else(|| Span::new(0, 0));

        let message: String = diagnostic.to_string();

        return Some(CompileError::new(context, span, message));
    }

    if parser_return.fatal_error {
        let span: Span = Span::new(0, 0);
        return Some(CompileError::new(
            context,
            span,
            "the parser aborted".to_string(),
        ));
    }

    None
}

/// Parse source code into a [`Program`].
pub fn parse<'ctx, 'a>(
    options: ParseOptions<'ctx, 'a>
) -> Result<ParseResult<'a>, CompileError> {
    let source_type: OxcSourceType = resolve_source_type(
        options.file,
        options.language,
        options.source_type,
    );

    let parser_return: oxc::parser::ParserReturn<'_> =
        Parser::new(options.allocator, options.code, source_type).parse();

    if let Some(error) = parse_error(options.context, &parser_return) {
        return Err(error);
    }

    Ok(ParseResult { program: parser_return.program })
}

/// Parse source code into an owned [`Ast`].
pub fn parse_owned(
    options: ParseOwnedOptions<'_, '_>
) -> Result<Ast, CompileError> {
    let source_type: OxcSourceType = resolve_source_type(
        options.file,
        options.language,
        options.source_type,
    );

    let source: Arc<str> = Arc::from(options.code);
    let context: &CompileContext<'_> = options.context;

    Ast::try_from_source(
        source,
        source_type,
        |src: &str, allocator: &Allocator| {
            let parser_return: oxc::parser::ParserReturn<'_> =
                Parser::new(allocator, src, source_type).parse();

            if let Some(error) = parse_error(context, &parser_return) {
                return Err(error);
            }

            Ok(parser_return.program)
        },
    )
}

#[cfg(test)]
mod tests {
    use oxc::allocator::Allocator;

    use super::*;

    const CWD: &str = "";

    const FILE: &str = "index.ts";

    fn parse_code_with_options<'a>(
        allocator: &'a Allocator,
        file: &'a str,
        language: Option<Language>,
        source_type: Option<SourceType>,
        code: &'a str,
    ) -> Result<ParseResult<'a>, CompileError> {
        let context: CompileContext<'_> = CompileContext::new(CWD, file, code);

        let options: ParseOptions<'_, 'a> = ParseOptions {
            context: &context,
            allocator,
            file,
            code,
            language,
            source_type,
        };

        parse(options)
    }

    fn parse_code<'a>(
        allocator: &'a Allocator,
        file: &'a str,
        code: &'a str,
    ) -> Result<ParseResult<'a>, CompileError> {
        parse_code_with_options(allocator, file, None, None, code)
    }

    fn parse_owned_code(
        file: &str,
        language: Option<Language>,
        source_type: Option<SourceType>,
        code: &str,
    ) -> Result<Ast, CompileError> {
        let context: CompileContext<'_> = CompileContext::new(CWD, file, code);

        let options: ParseOwnedOptions<'_, '_> = ParseOwnedOptions {
            context: &context,
            file,
            code,
            language,
            source_type,
        };

        parse_owned(options)
    }

    #[test]
    fn test_parse_owned_ok() {
        let ast: Ast = parse_owned_code(FILE, None, None, "const a = 1;")
            .expect("valid source parses");

        assert!(!ast.program().body.is_empty());
        assert!(ast.program().source_type.is_typescript());
        assert_eq!(ast.source().as_ref(), "const a = 1;");
    }

    #[test]
    fn test_parse_owned_source_text_borrows_owned_source() {
        let ast: Ast = parse_owned_code(FILE, None, None, "const a = 1;")
            .expect("valid source parses");

        assert_eq!(ast.program().source_text, ast.source().as_ref());
    }

    #[test]
    fn test_parse_owned_invalid_syntax_errors() {
        let result: Result<Ast, CompileError> =
            parse_owned_code(FILE, None, None, "const = ;");

        let error: CompileError = result.expect_err("invalid syntax errors");

        assert!(error.to_string().contains(FILE));
    }

    #[test]
    fn test_parse_owned_resolves_explicit_language() {
        let ast: Ast = parse_owned_code(
            "app.js",
            Some(Language::TS),
            None,
            "const a: number = 1;",
        )
        .expect("explicit language parses");

        assert!(ast.program().source_type.is_typescript());
    }

    #[test]
    fn test_parse_owned_resolves_explicit_module_kind() {
        let ast: Ast = parse_owned_code(
            FILE,
            Some(Language::JS),
            Some(SourceType::Script),
            "const a = 1;",
        )
        .expect("script module kind parses");

        assert!(ast.program().source_type.is_script());
    }

    #[test]
    fn test_parse_owned_clone_round_trips() {
        let ast: Ast = parse_owned_code(FILE, None, None, "const a = 1;")
            .expect("valid source parses");

        let clone: Ast = ast.clone();

        assert_eq!(clone.program().body.len(), ast.program().body.len());
        assert_eq!(clone.program().source_text, ast.program().source_text);
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

    #[test]
    fn test_parse_tsx_source_type() {
        let allocator: Allocator = Allocator::default();
        let file: &str = "index.tsx";
        let result: ParseResult<'_> =
            parse_code(&allocator, file, "const x = <div>hi</div>;")
                .expect("tsx source parses");

        assert!(result.program.source_type.is_typescript());
        assert!(result.program.source_type.is_jsx());
    }

    #[test]
    fn test_parse_jsx_source_type() {
        let allocator: Allocator = Allocator::default();
        let file: &str = "index.jsx";
        let result: ParseResult<'_> =
            parse_code(&allocator, file, "const x = <div>hi</div>;")
                .expect("jsx source parses");

        assert!(result.program.source_type.is_javascript());
        assert!(result.program.source_type.is_jsx());
        assert!(!result.program.source_type.is_typescript());
    }

    #[test]
    fn test_parse_explicit_language_overrides_extension() {
        let allocator: Allocator = Allocator::default();

        let file: &str = "app.js";

        let result: ParseResult<'_> = parse_code_with_options(
            &allocator,
            file,
            Some(Language::TS),
            None,
            "const a: number = 1;",
        )
        .expect("explicit language parses");

        assert!(result.program.source_type.is_typescript());
        assert!(!result.program.source_type.is_jsx());
    }

    #[test]
    fn test_parse_explicit_jsx_parses_without_extension() {
        let allocator: Allocator = Allocator::default();

        let file: &str = "app";

        let result: ParseResult<'_> = parse_code_with_options(
            &allocator,
            file,
            Some(Language::JSX),
            None,
            "const x = <div>hi</div>;",
        )
        .expect("explicit jsx language parses");

        assert!(result.program.source_type.is_javascript());
        assert!(result.program.source_type.is_jsx());
    }

    #[test]
    fn test_parse_explicit_dts_parses_declarations() {
        let allocator: Allocator = Allocator::default();

        let file: &str = "app.js";

        let result: ParseResult<'_> = parse_code_with_options(
            &allocator,
            file,
            Some(Language::DTS),
            None,
            "declare const a: string;",
        )
        .expect("explicit dts language parses");

        assert!(result.program.source_type.is_typescript_definition());
        assert!(!result.program.source_type.is_jsx());
    }

    #[test]
    fn test_parse_module_kind_script_overrides_unambiguous() {
        let allocator: Allocator = Allocator::default();

        let result: ParseResult<'_> = parse_code_with_options(
            &allocator,
            FILE,
            Some(Language::JS),
            Some(SourceType::Script),
            "const a = 1;",
        )
        .expect("script module kind parses");

        assert!(result.program.source_type.is_script());
        assert!(!result.program.source_type.is_module());
        assert!(!result.program.source_type.is_unambiguous());
    }

    #[test]
    fn test_parse_module_kind_module_on_typescript() {
        let allocator: Allocator = Allocator::default();

        let result: ParseResult<'_> = parse_code_with_options(
            &allocator,
            FILE,
            Some(Language::TS),
            Some(SourceType::Module),
            "const a = 1;",
        )
        .expect("module module kind parses");

        assert!(result.program.source_type.is_module());
        assert!(result.program.source_type.is_typescript());
    }

    #[test]
    fn test_parse_module_kind_overrides_grammar_from_path() {
        let allocator: Allocator = Allocator::default();

        let file: &str = "index.ts";

        let result: ParseResult<'_> = parse_code_with_options(
            &allocator,
            file,
            None,
            Some(SourceType::Script),
            "const a = 1;",
        )
        .expect("script override on path-inferred grammar parses");

        assert!(result.program.source_type.is_typescript());
        assert!(result.program.source_type.is_script());
    }

    #[test]
    fn test_parse_module_kind_unambiguous_pins_default() {
        let allocator: Allocator = Allocator::default();

        let result: ParseResult<'_> = parse_code_with_options(
            &allocator,
            FILE,
            Some(Language::TS),
            Some(SourceType::Unambiguous),
            "export const a = 1;",
        )
        .expect("unambiguous module kind parses");

        // The parser resolves an unambiguous source to script or module;
        // ESM syntax resolves to module.
        assert!(result.program.source_type.is_module());
        assert!(result.program.source_type.is_typescript());
    }

    #[test]
    fn test_grammar_source_type() {
        let js: OxcSourceType = grammar_source_type(Language::JS);

        assert!(js.is_javascript());
        assert!(!js.is_typescript());
        assert!(!js.is_jsx());
        assert!(js.is_unambiguous());

        let ts: OxcSourceType = grammar_source_type(Language::TS);

        assert!(ts.is_typescript());
        assert!(!ts.is_javascript());
        assert!(!ts.is_jsx());
        assert!(ts.is_unambiguous());

        let dts: OxcSourceType = grammar_source_type(Language::DTS);

        assert!(dts.is_typescript());
        assert!(dts.is_typescript_definition());
        assert!(!dts.is_jsx());

        let jsx: OxcSourceType = grammar_source_type(Language::JSX);

        assert!(jsx.is_javascript());
        assert!(!jsx.is_typescript());
        assert!(jsx.is_jsx());
        assert!(jsx.is_unambiguous());

        let tsx: OxcSourceType = grammar_source_type(Language::TSX);

        assert!(tsx.is_typescript());
        assert!(!tsx.is_javascript());
        assert!(tsx.is_jsx());
        assert!(tsx.is_unambiguous());
    }

    #[test]
    fn test_with_module_kind() {
        let grammar: OxcSourceType = grammar_source_type(Language::JS);

        let script: OxcSourceType =
            with_module_kind(grammar, SourceType::Script);

        assert!(script.is_script());
        assert!(!script.is_module());
        assert!(!script.is_commonjs());
        assert!(!script.is_unambiguous());

        let commonjs: OxcSourceType =
            with_module_kind(grammar, SourceType::CommonJS);

        assert!(commonjs.is_commonjs());
        assert!(!commonjs.is_module());
        assert!(!commonjs.is_script());

        let module: OxcSourceType =
            with_module_kind(grammar, SourceType::Module);

        assert!(module.is_module());
        assert!(!module.is_script());
        assert!(!module.is_commonjs());

        let unambiguous: OxcSourceType =
            with_module_kind(grammar, SourceType::Unambiguous);

        assert!(unambiguous.is_unambiguous());
        assert!(!unambiguous.is_script());
        assert!(!unambiguous.is_module());
    }
}
