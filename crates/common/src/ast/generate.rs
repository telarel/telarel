use std::path::PathBuf;

use oxc::ast::ast::Program;
use oxc::codegen::{Codegen, CodegenOptions as OxcCodegenOptions};

use crate::ast::ast::Ast;
use crate::ast::result::CodeResult;
use crate::sourcemap::{SourceMap, compose::compose_maps};

/// Options for [`generate_code`].
#[derive(Debug, Clone)]
pub struct GenerateCodeOptions {
    /// The file being generated.
    pub file: String,
}

/// Strip a trailing `\n`, then a trailing `\r`, so `\n`, `\r\n`, and
/// lone `\r` terminators disappear while interior line endings survive.
///
/// Input is owned, so the terminator is removed in place rather than
/// copying the whole string into a fresh `String`.
fn strip_trailing_newline(mut source: String) -> String {
    let new_len: usize =
        source.trim_end_matches('\n').trim_end_matches('\r').len();

    source.truncate(new_len);

    source
}

/// Generate code + map from an owned AST, composing the AST's input map
/// into the produced map. Always produces a map.
pub fn generate_code(
    ast: &Ast,
    options: GenerateCodeOptions,
) -> CodeResult {
    let (code, codegen_map): (String, SourceMap) =
        generate_program(ast.program(), &options.file);

    let code: String = strip_trailing_newline(code);

    let map: SourceMap = match ast.map() {
        | Some(input_map) => {
            compose_maps(Some(&options.file), &codegen_map, &[input_map])
        },
        | None => codegen_map,
    };

    CodeResult { code, map: Some(map) }
}

/// Low-level program codegen (raw oxc output, no newline strip, no
/// composition). Retained for internal consumers (plugin_transform change
/// detection).
pub fn generate_program(
    program: &Program<'_>,
    file: &str,
) -> (String, SourceMap) {
    let codegen_options: OxcCodegenOptions = OxcCodegenOptions {
        source_map_path: Some(PathBuf::from(file)),
        ..OxcCodegenOptions::default()
    };

    let result: oxc::codegen::CodegenReturn<'_> =
        Codegen::new().with_options(codegen_options).build(program);

    let map: SourceMap = result
        .map
        .expect("oxc produces a source map when source_map_path is set")
        .into_owned();

    (result.code, map)
}

#[cfg(test)]
mod tests {
    use oxc_sourcemap::SourceMapBuilder;

    use crate::ast::parse::{ParseAstOptions, parse_ast};

    use super::*;

    const FILE: &str = "index.ts";

    fn parse_code(code: &str) -> Ast {
        let options: ParseAstOptions = ParseAstOptions {
            cwd: PathBuf::new(),
            file: FILE.to_string(),
            language: None,
            source_type: None,
            input_map: None,
        };

        parse_ast(code.to_string(), options).expect("valid source parses")
    }

    fn sample_input_map() -> SourceMap {
        let mut builder: SourceMapBuilder<'_> = SourceMapBuilder::default();

        builder.set_file("upstream.ts");

        let source_id: u32 =
            builder.add_source_and_content("upstream.ts", "const a = 1;");

        // original line 9 col 0 was produced into parse-input (0, 0)
        builder.add_token(0, 0, 9, 0, Some(source_id), None);

        builder.into_owned_sourcemap().into_inner()
    }

    #[test]
    fn test_generate_code_produces_code() {
        let ast: Ast = parse_code("const a = 1;");

        let result: CodeResult =
            generate_code(&ast, GenerateCodeOptions { file: FILE.to_string() });

        assert!(!result.code.is_empty());
    }

    #[test]
    fn test_generate_code_produces_sourcemap() {
        let ast: Ast = parse_code("const a = 1;");

        let result: CodeResult =
            generate_code(&ast, GenerateCodeOptions { file: FILE.to_string() });

        let map: SourceMap = result.map.expect("map always produced");
        let json: String = map.to_json_string();

        assert!(json.contains("\"mappings\":\""));
        assert!(!json.contains("\"mappings\":\"\""));
    }

    #[test]
    fn test_generate_code_sourcemap_sources_entry() {
        let ast: Ast = parse_code("const a = 1;");

        let result: CodeResult =
            generate_code(&ast, GenerateCodeOptions { file: FILE.to_string() });

        let map: SourceMap = result.map.expect("map always produced");

        assert_eq!(map.get_source(0), Some(FILE));
    }

    #[test]
    fn test_generate_code_roundtrip_preserves_statements() {
        let code: &str =
            "const a = 1; let b = 2; function f() { return a + b; }";
        let ast: Ast = parse_code(code);

        let result: CodeResult =
            generate_code(&ast, GenerateCodeOptions { file: FILE.to_string() });

        let compact: String = result.code.split_whitespace().collect();

        assert!(compact.contains("consta=1"));
        assert!(compact.contains("letb=2"));
        assert!(compact.contains("functionf()"));
    }

    #[test]
    fn test_generate_code_strips_trailing_newline() {
        let ast: Ast = parse_code("const a = 1;");

        let result: CodeResult =
            generate_code(&ast, GenerateCodeOptions { file: FILE.to_string() });

        assert_eq!(result.code, "const a = 1;");
    }

    #[test]
    fn test_generate_code_without_input_map_uses_codegen_map() {
        let ast: Ast = parse_code("const a = 1;");

        let result: CodeResult =
            generate_code(&ast, GenerateCodeOptions { file: FILE.to_string() });

        let map: SourceMap = result.map.expect("map always produced");

        let token: oxc_sourcemap::Token =
            map.get_token(0).expect("first token exists");

        assert_eq!(token.get_src_line(), 0);
        assert_eq!(token.get_src_col(), 0);
    }

    #[test]
    fn test_generate_code_composes_input_map() {
        let options: ParseAstOptions = ParseAstOptions {
            cwd: PathBuf::new(),
            file: FILE.to_string(),
            language: None,
            source_type: None,
            input_map: Some(sample_input_map()),
        };

        let ast: Ast =
            parse_ast("const a = 1;".to_string(), options).expect("parses");

        let result: CodeResult =
            generate_code(&ast, GenerateCodeOptions { file: FILE.to_string() });

        let map: SourceMap = result.map.expect("map always produced");

        // The input map resolves parse-input (0, 0) to original (9, 0);
        // composition must carry that through the codegen map's src side.
        let token: oxc_sourcemap::Token =
            map.get_token(0).expect("first token exists");

        assert_eq!(token.get_dst_line(), 0);
        assert_eq!(token.get_dst_col(), 0);
        assert_eq!(token.get_src_line(), 9);
        assert_eq!(token.get_src_col(), 0);
    }

    #[test]
    fn test_generate_code_uses_file_as_map_file() {
        let options: ParseAstOptions = ParseAstOptions {
            cwd: PathBuf::new(),
            file: FILE.to_string(),
            language: None,
            source_type: None,
            input_map: Some(sample_input_map()),
        };

        let ast: Ast =
            parse_ast("const a = 1;".to_string(), options).expect("parses");

        let file: &str = "entry.ts?v=1#h";

        let result: CodeResult =
            generate_code(&ast, GenerateCodeOptions { file: file.to_string() });

        let map: SourceMap = result.map.expect("map always produced");

        assert_eq!(map.get_file(), Some(file));
    }
}
