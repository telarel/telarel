use std::path::PathBuf;
use std::sync::Arc;

use oxc::allocator::Allocator;
use oxc::ast::ast::Program;
use oxc::diagnostics::LabeledSpan;
use oxc::parser::Parser;
use oxc::span::{SourceType as OxcSourceType, Span};

use crate::_types::grammar::language::Language;
use crate::_types::grammar::source_type::{SourceType, with_module_kind};
use crate::ast::ast::Ast;
use crate::contexts::compile::CompileContext;
use crate::errors::compile::CompileError;
use crate::helpers::resolve::resolve_grammar;
use crate::sourcemap::SourceMap;

/// Options for [`parse_program`].
#[derive(Clone, Copy)]
pub struct ParseProgramOptions<'ctx, 'a> {
    /// Compile context for diagnostics.
    pub context: &'ctx CompileContext<'ctx>,
    /// Destination allocator for the parsed program.
    pub allocator: &'a Allocator,
    /// The file being parsed.
    pub file: &'a str,
    /// The grammar of the code;
    /// `None` infers the grammar from the file extension.
    pub language: Option<Language>,
    /// The module system of the code;
    /// `None` keeps the module kind resolved from the grammar.
    pub source_type: Option<SourceType>,
}

/// Result of [`parse_program`].
#[derive(Debug)]
pub struct ParseProgramResult<'a> {
    /// Parsed program, borrowed from the allocator passed to [`parse_program`].
    pub program: Program<'a>,
}

/// Options for [`parse_ast`].
#[derive(Debug, Clone)]
pub struct ParseAstOptions {
    /// Working directory used to resolve relative paths.
    pub cwd: PathBuf,
    /// The file being parsed.
    pub file: String,
    /// The grammar of the code;
    /// `None` infers the grammar from the file extension.
    pub language: Option<Language>,
    /// The module system of the code;
    /// `None` keeps the module kind resolved from the grammar.
    pub source_type: Option<SourceType>,
    /// The input source map describing how `code` was produced from
    /// upstream originals, if any.
    pub input_map: Option<SourceMap>,
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

        return Some(CompileError::new(message).with_source_code(
            context.file,
            context.code,
            span,
        ));
    }

    if parser_return.fatal_error {
        let span: Span = Span::new(0, 0);

        return Some(CompileError::new("the parser aborted").with_source_code(
            context.file,
            context.code,
            span,
        ));
    }

    None
}

/// Parse source code into a [`Program`] borrowed from `allocator`.
pub fn parse_program<'ctx, 'a>(
    code: &'a str,
    options: ParseProgramOptions<'ctx, 'a>,
) -> Result<ParseProgramResult<'a>, CompileError> {
    let grammar: OxcSourceType =
        resolve_grammar(options.file, options.language);

    let source_type: OxcSourceType = match options.source_type {
        | Some(module_kind) => with_module_kind(grammar, module_kind),
        | None => grammar,
    };

    let parser_return: oxc::parser::ParserReturn<'_> =
        Parser::new(options.allocator, code, source_type).parse();

    if let Some(error) = parse_error(options.context, &parser_return) {
        return Err(error);
    }

    Ok(ParseProgramResult { program: parser_return.program })
}

/// Parse source code into an owned [`Ast`].
pub fn parse_ast(
    code: String,
    options: ParseAstOptions,
) -> Result<Ast, CompileError> {
    let grammar: OxcSourceType =
        resolve_grammar(&options.file, options.language);

    let source_type: OxcSourceType = match options.source_type {
        | Some(module_kind) => with_module_kind(grammar, module_kind),
        | None => grammar,
    };

    let source: Arc<str> = Arc::from(code.as_str());

    let context: CompileContext<'_> =
        CompileContext::new(&options.cwd, &options.file, &code);

    Ast::try_from_source(
        source,
        source_type,
        options.input_map,
        |src: &str, allocator: &Allocator| {
            let parser_return: oxc::parser::ParserReturn<'_> =
                Parser::new(allocator, src, source_type).parse();

            if let Some(error) = parse_error(&context, &parser_return) {
                return Err(error);
            }

            Ok(parser_return.program)
        },
    )
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use oxc::allocator::Allocator;
    use oxc_sourcemap::SourceMapBuilder;

    use crate::_types::grammar::language::grammar_source_type;

    use super::*;

    const FILE: &str = "index.ts";

    fn parse_code_with_options<'a>(
        allocator: &'a Allocator,
        file: &'a str,
        language: Option<Language>,
        source_type: Option<SourceType>,
        code: &'a str,
    ) -> Result<ParseProgramResult<'a>, CompileError> {
        let context: CompileContext<'_> =
            CompileContext::new(Path::new(""), file, code);

        let options: ParseProgramOptions<'_, 'a> = ParseProgramOptions {
            context: &context,
            allocator,
            file,
            language,
            source_type,
        };

        parse_program(code, options)
    }

    fn parse_code<'a>(
        allocator: &'a Allocator,
        file: &'a str,
        code: &'a str,
    ) -> Result<ParseProgramResult<'a>, CompileError> {
        parse_code_with_options(allocator, file, None, None, code)
    }

    fn parse_ast_code(
        file: &str,
        language: Option<Language>,
        source_type: Option<SourceType>,
        code: &str,
    ) -> Result<Ast, CompileError> {
        let options: ParseAstOptions = ParseAstOptions {
            cwd: PathBuf::new(),
            file: file.to_string(),
            language,
            source_type,
            input_map: None,
        };

        parse_ast(code.to_string(), options)
    }

    #[test]
    fn test_parse_ast_ok() {
        let ast: Ast = parse_ast_code(FILE, None, None, "const a = 1;")
            .expect("valid source parses");

        assert!(!ast.program().body.is_empty());
        assert!(ast.program().source_type.is_typescript());
        assert_eq!(ast.source().as_ref(), "const a = 1;");
    }

    #[test]
    fn test_parse_ast_source_text_borrows_owned_source() {
        let ast: Ast = parse_ast_code(FILE, None, None, "const a = 1;")
            .expect("valid source parses");

        assert_eq!(ast.program().source_text, ast.source().as_ref());
    }

    #[test]
    fn test_parse_ast_invalid_syntax_errors() {
        let result: Result<Ast, CompileError> =
            parse_ast_code(FILE, None, None, "const = ;");

        let error: CompileError = result.expect_err("invalid syntax errors");

        assert!(error.to_string().contains(FILE));
    }

    #[test]
    fn test_parse_ast_attaches_input_map() {
        let mut builder: SourceMapBuilder<'_> = SourceMapBuilder::default();

        builder.set_file("upstream.ts");

        let source_id: u32 =
            builder.add_source_and_content("upstream.ts", "const a = 1;");

        builder.add_token(0, 0, 0, 0, Some(source_id), None);

        let input_map: SourceMap = builder.into_owned_sourcemap().into_inner();

        let options: ParseAstOptions = ParseAstOptions {
            cwd: PathBuf::new(),
            file: FILE.to_string(),
            language: None,
            source_type: None,
            input_map: Some(input_map.clone()),
        };

        let ast: Ast =
            parse_ast("const a = 1;".to_string(), options).expect("parses");

        let attached: &SourceMap = ast.map().expect("input map attached");

        assert_eq!(attached.to_json_string(), input_map.to_json_string());
    }

    #[test]
    fn test_parse_ast_without_input_map_attaches_none() {
        let ast: Ast = parse_ast_code(FILE, None, None, "const a = 1;")
            .expect("valid source parses");

        assert!(ast.map().is_none());
    }

    #[test]
    fn test_parse_ast_resolves_explicit_language() {
        let ast: Ast = parse_ast_code(
            "app.js",
            Some(Language::TS),
            None,
            "const a: number = 1;",
        )
        .expect("explicit language parses");

        assert!(ast.program().source_type.is_typescript());
    }

    #[test]
    fn test_parse_ast_resolves_explicit_module_kind() {
        let ast: Ast = parse_ast_code(
            FILE,
            Some(Language::JS),
            Some(SourceType::Script),
            "const a = 1;",
        )
        .expect("script module kind parses");

        assert!(ast.program().source_type.is_script());
    }

    #[test]
    fn test_parse_ast_clone_round_trips() {
        let ast: Ast = parse_ast_code(FILE, None, None, "const a = 1;")
            .expect("valid source parses");

        let clone: Ast = ast.clone();

        assert_eq!(clone.program().body.len(), ast.program().body.len());
        assert_eq!(clone.program().source_text, ast.program().source_text);
    }

    #[test]
    fn test_parse_ok() {
        let allocator: Allocator = Allocator::default();
        let result: ParseProgramResult<'_> =
            parse_code(&allocator, FILE, "const a = 1;")
                .expect("valid source parses");

        assert!(!result.program.body.is_empty());
        assert!(result.program.source_type.is_typescript());
    }

    #[test]
    fn test_parse_invalid_syntax_errors() {
        let allocator: Allocator = Allocator::default();
        let result: Result<ParseProgramResult<'_>, CompileError> =
            parse_code(&allocator, FILE, "const = ;");

        assert!(result.is_err());
    }

    #[test]
    fn test_parse_strips_query_suffix() {
        let allocator: Allocator = Allocator::default();
        let file: &str = "index.js?v=d48eb52a";
        let result: ParseProgramResult<'_> =
            parse_code(&allocator, file, "const x = 1;")
                .expect("query suffix is stripped");

        assert!(result.program.source_type.is_javascript());
        assert!(!result.program.source_type.is_typescript());
    }

    #[test]
    fn test_parse_strips_hash_suffix() {
        let allocator: Allocator = Allocator::default();
        let file: &str = "index.ts#hash";
        let result: ParseProgramResult<'_> =
            parse_code(&allocator, file, "const x = 1;")
                .expect("hash suffix is stripped");

        assert!(result.program.source_type.is_typescript());
    }

    #[test]
    fn test_parse_strips_query_and_hash() {
        let allocator: Allocator = Allocator::default();
        let file: &str = "index.tsx?v=1#h";
        let result: ParseProgramResult<'_> =
            parse_code(&allocator, file, "const x = 1;")
                .expect("query and hash suffixes are stripped");

        assert!(result.program.source_type.is_typescript());
    }

    #[test]
    fn test_parse_strips_hash_before_query() {
        let allocator: Allocator = Allocator::default();
        let file: &str = "index.ts#h?v=1";
        let result: ParseProgramResult<'_> =
            parse_code(&allocator, file, "const x = 1;")
                .expect("hash-before-query suffix is stripped");

        assert!(result.program.source_type.is_typescript());
    }

    #[test]
    fn test_parse_unknown_extension_falls_back_to_esm() {
        let allocator: Allocator = Allocator::default();
        let file: &str = "index.unknown";
        let result: Result<ParseProgramResult<'_>, CompileError> =
            parse_code(&allocator, file, "const x = 1;");

        assert!(result.is_ok());

        let parsed: ParseProgramResult<'_> =
            result.expect("ESM fallback parses");

        assert!(parsed.program.source_type.is_javascript());
        assert!(!parsed.program.source_type.is_typescript());
    }

    #[test]
    fn test_parse_tsx_source_type() {
        let allocator: Allocator = Allocator::default();
        let file: &str = "index.tsx";
        let result: ParseProgramResult<'_> =
            parse_code(&allocator, file, "const x = <div>hi</div>;")
                .expect("tsx source parses");

        assert!(result.program.source_type.is_typescript());
        assert!(result.program.source_type.is_jsx());
    }

    #[test]
    fn test_parse_jsx_source_type() {
        let allocator: Allocator = Allocator::default();
        let file: &str = "index.jsx";
        let result: ParseProgramResult<'_> =
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

        let result: ParseProgramResult<'_> = parse_code_with_options(
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

        let result: ParseProgramResult<'_> = parse_code_with_options(
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

        let result: ParseProgramResult<'_> = parse_code_with_options(
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

        let result: ParseProgramResult<'_> = parse_code_with_options(
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

        let result: ParseProgramResult<'_> = parse_code_with_options(
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

        let result: ParseProgramResult<'_> = parse_code_with_options(
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

        let result: ParseProgramResult<'_> = parse_code_with_options(
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
