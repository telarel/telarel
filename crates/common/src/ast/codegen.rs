use std::path::PathBuf;

use oxc::ast::ast::Program;
use oxc::codegen::{Codegen, CodegenOptions as OxcCodegenOptions};
use oxc_sourcemap::SourceMap;

/// Options for [`codegen`].
#[derive(Debug, Clone, Copy)]
pub struct CodegenOptions<'f, 'a> {
    /// File path.
    pub file: &'f str,
    /// Program to print.
    pub program: &'a Program<'a>,
}

/// Result of [`codegen`].
#[derive(Debug, Clone)]
pub struct CodegenResult<'a> {
    /// Generated code.
    pub code: String,
    /// Source map (always present: source_map_path is set).
    pub map: SourceMap<'a>,
}

/// Print a program, always producing a source map.
pub fn codegen<'f, 'a>(options: CodegenOptions<'f, 'a>) -> CodegenResult<'a> {
    let codegen_options: OxcCodegenOptions = OxcCodegenOptions {
        source_map_path: Some(PathBuf::from(options.file)),
        ..OxcCodegenOptions::default()
    };

    let result: oxc::codegen::CodegenReturn<'_> =
        Codegen::new().with_options(codegen_options).build(options.program);

    let map: SourceMap<'a> = result
        .map
        .expect("oxc produces a source map when source_map_path is set");

    CodegenResult { code: result.code, map }
}

#[cfg(test)]
mod tests {
    use oxc::allocator::Allocator;

    use crate::ast::parse::{ParseOptions, ParseResult, parse};
    use crate::contexts::compile::CompileContext;

    use super::*;

    const CWD: &str = "";

    const FILE: &str = "index.ts";

    fn parse_code<'a>(
        allocator: &'a Allocator,
        code: &'a str,
    ) -> ParseResult<'a> {
        let context: CompileContext<'_> = CompileContext::new(CWD, FILE, code);

        let options: ParseOptions<'_, 'a> =
            ParseOptions { context: &context, allocator, file: FILE, code };

        parse(options).expect("valid source parses")
    }

    #[test]
    fn test_codegen_produces_code() {
        let allocator: Allocator = Allocator::default();
        let code: &str = "const a = 1;";
        let parsed: ParseResult<'_> = parse_code(&allocator, code);

        let result: CodegenResult<'_> =
            codegen(CodegenOptions { file: FILE, program: &parsed.program });

        assert!(!result.code.is_empty());
    }

    #[test]
    fn test_codegen_produces_sourcemap() {
        let allocator: Allocator = Allocator::default();
        let code: &str = "const a = 1;";
        let parsed: ParseResult<'_> = parse_code(&allocator, code);

        let result: CodegenResult<'_> =
            codegen(CodegenOptions { file: FILE, program: &parsed.program });

        let json: String = result.map.to_json_string();

        assert!(json.contains("\"mappings\":\""));
        assert!(!json.contains("\"mappings\":\"\""));
    }

    #[test]
    fn test_codegen_sourcemap_sources_entry() {
        let allocator: Allocator = Allocator::default();
        let code: &str = "const a = 1;";
        let parsed: ParseResult<'_> = parse_code(&allocator, code);

        let result: CodegenResult<'_> =
            codegen(CodegenOptions { file: FILE, program: &parsed.program });

        let source: Option<&str> = result.map.get_source(0);

        assert_eq!(source, Some(FILE));
    }

    #[test]
    fn test_codegen_roundtrip_preserves_statements() {
        let allocator: Allocator = Allocator::default();
        let code: &str =
            "const a = 1; let b = 2; function f() { return a + b; }";
        let parsed: ParseResult<'_> = parse_code(&allocator, code);

        let result: CodegenResult<'_> =
            codegen(CodegenOptions { file: FILE, program: &parsed.program });

        let compact: String = result.code.split_whitespace().collect();

        assert!(compact.contains("consta=1"));
        assert!(compact.contains("letb=2"));
        assert!(compact.contains("functionf()"));
    }
}
