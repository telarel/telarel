use std::path::PathBuf;

use oxc::ast::ast::Program;
use oxc::codegen::Codegen;

use crate::_types::options::OxcCodegenOptions;
use crate::ast::ast::Ast;
use crate::ast::result::CodeResult;
use crate::helpers::resolve::default_codegen_oxc;
use crate::sourcemap::SourceMap;

/// Options for [`generate_code`].
#[derive(Debug, Clone)]
pub struct GenerateCodeOptions {
    /// The file being generated.
    pub file: String,
}

/// Input is owned, so the terminator is removed in place rather than copying
/// the whole string into a fresh `String`.
fn strip_trailing_newline(mut source: String) -> String {
    let new_len: usize =
        source.trim_end_matches('\n').trim_end_matches('\r').len();

    source.truncate(new_len);

    source
}

/// Generate code + map from an owned AST, composing the AST's input map
/// into the produced map. The Telarel defaults back the codegen options. The
/// function always produces a map and records `options.file` as its file.
pub fn generate_code(
    ast: &Ast,
    options: GenerateCodeOptions,
) -> CodeResult {
    generate_code_with_codegen(ast, options, default_codegen_oxc())
}

/// Generate code + map from an owned AST with explicit raw oxc codegen
/// options.
///
/// Telarel defaults are not applied: `codegen` is used verbatim. Use
/// [`generate_code`] for the Telarel defaults. The AST's input map is composed
/// into the produced map. The function always produces a map and records
/// `options.file` as its file. The function ignores `codegen.source_map_path`;
/// the file comes from `options`.
pub fn generate_code_with_codegen(
    ast: &Ast,
    options: GenerateCodeOptions,
    codegen: OxcCodegenOptions,
) -> CodeResult {
    let (code, codegen_map): (String, SourceMap) =
        generate_program_with_codegen(ast.program(), &options.file, codegen);

    let code: String = strip_trailing_newline(code);

    let mut map: SourceMap = match ast.map() {
        | Some(input_map) => codegen_map.compose(input_map.clone()),
        | None => codegen_map,
    };

    // oxc codegen leaves `file` unset and `compose` keeps the input map's
    // `file`, so record the generated file after `compose`.
    map.set_file(&options.file);

    CodeResult { code, map: Some(map) }
}

/// Low-level program codegen (raw oxc output, no newline strip, no
/// composition). The Telarel defaults back the codegen options. Retained for
/// internal consumers (plugin_transform change detection).
pub fn generate_program(
    program: &Program<'_>,
    file: &str,
) -> (String, SourceMap) {
    generate_program_with_codegen(program, file, default_codegen_oxc())
}

/// Low-level program codegen with explicit raw oxc codegen options.
///
/// Telarel defaults are not applied: `codegen` is used verbatim. Use
/// [`generate_program`] for the Telarel defaults. The function ignores
/// `codegen.source_map_path`. The map records `file` instead.
pub fn generate_program_with_codegen(
    program: &Program<'_>,
    file: &str,
    mut codegen: OxcCodegenOptions,
) -> (String, SourceMap) {
    codegen.source_map_path = Some(PathBuf::from(file));

    let result: oxc::codegen::CodegenReturn<'_> =
        Codegen::new().with_options(codegen).build(program);

    let map: SourceMap = result
        .map
        .expect("oxc produces a source map when source_map_path is set")
        .into_owned();

    (result.code, map)
}

#[cfg(test)]
mod tests {
    use oxc::allocator::Allocator;
    use oxc::ast::ast::{BindingPattern, Ident, Statement, VariableDeclarator};
    use oxc::codegen::CommentOptions;
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

    fn multi_source_input_map() -> SourceMap {
        let mut builder: SourceMapBuilder<'_> = SourceMapBuilder::default();

        builder.set_file("bundle.ts");

        let a_id: u32 = builder.add_source_and_content("a.ts", "aaa");
        let b_id: u32 = builder.add_source_and_content("b.ts", "bbb");

        builder.add_token(0, 0, 9, 0, Some(a_id), None);
        builder.add_token(0, 10, 20, 5, Some(b_id), None);
        builder.add_token(1, 0, 40, 2, Some(b_id), None);
        builder.add_token(1, 10, 50, 7, Some(a_id), None);

        builder.into_owned_sourcemap().into_inner()
    }

    fn named_input_map() -> SourceMap {
        let mut builder: SourceMapBuilder<'_> = SourceMapBuilder::default();

        builder.set_file("upstream.ts");

        let source_id: u32 =
            builder.add_source_and_content("upstream.ts", "const a = 1;");
        let name_id: u32 = builder.add_name("upstreamName");

        builder.add_token(0, 0, 9, 0, Some(source_id), Some(name_id));

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
    fn test_generate_code_produces_source_map() {
        let ast: Ast = parse_code("const a = 1;");

        let result: CodeResult =
            generate_code(&ast, GenerateCodeOptions { file: FILE.to_string() });

        let map: SourceMap = result.map.expect("map always produced");
        let json: String = map.to_json_string();

        assert!(json.contains("\"mappings\":\""));
        assert!(!json.contains("\"mappings\":\"\""));
    }

    #[test]
    fn test_generate_code_source_map_sources_entry() {
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
    fn test_generate_code_uses_telarel_default_indentation() {
        let ast: Ast = parse_code("function f() {\nreturn;\n}");

        let result: CodeResult =
            generate_code(&ast, GenerateCodeOptions { file: FILE.to_string() });

        assert_eq!(result.code, "function f() {\n  return;\n}");
    }

    #[test]
    fn test_generate_code_without_input_map_uses_codegen_map() {
        let ast: Ast = parse_code("const a = 1;");

        let result: CodeResult =
            generate_code(&ast, GenerateCodeOptions { file: FILE.to_string() });

        let map: SourceMap = result.map.expect("map always produced");

        assert_eq!(map.get_file(), Some(FILE));

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

        // The input map resolves parse-input (0, 0) to original (9, 0).
        // Composition must carry that through the codegen map's src side.
        let token: oxc_sourcemap::Token =
            map.get_token(0).expect("first token exists");

        assert_eq!(token.get_dst_line(), 0);
        assert_eq!(token.get_dst_col(), 0);
        assert_eq!(token.get_src_line(), 9);
        assert_eq!(token.get_src_col(), 0);

        assert_eq!(
            map.get_sources().collect::<Vec<Option<&str>>>(),
            vec![Some("upstream.ts")],
        );
        assert_eq!(
            map.get_source_contents().collect::<Vec<Option<&str>>>(),
            [Some("const a = 1;")],
        );
    }

    #[test]
    fn test_generate_code_composes_multi_source_input_map() {
        let options: ParseAstOptions = ParseAstOptions {
            cwd: PathBuf::new(),
            file: FILE.to_string(),
            language: None,
            source_type: None,
            input_map: Some(multi_source_input_map()),
        };

        let ast: Ast =
            parse_ast("const a = 1;\nconst b = 2;".to_string(), options)
                .expect("parses");

        let result: CodeResult =
            generate_code(&ast, GenerateCodeOptions { file: FILE.to_string() });

        let map: SourceMap = result.map.expect("map always produced");

        assert_eq!(map.get_file(), Some(FILE));
        assert_eq!(
            map.get_sources().collect::<Vec<Option<&str>>>(),
            vec![Some("a.ts"), Some("b.ts")],
        );
        assert_eq!(
            map.get_source_contents().collect::<Vec<Option<&str>>>(),
            [Some("aaa"), Some("bbb")],
        );

        assert_eq!(map.to_json().mappings, "AASA,MAAA,ICWK;AAoBH,MAAA,IDUK");

        // A codegen token that lands between input segments takes the input
        // segment's position. It is not offset by a delta.
        let between: oxc_sourcemap::Token =
            map.get_token(1).expect("second token exists");

        assert_eq!(
            (
                between.get_dst_line(),
                between.get_dst_col(),
                between.get_src_line(),
                between.get_src_col(),
            ),
            (0, 6, 9, 0),
        );

        // The source id follows the input map, not the codegen map.
        let b_source: oxc_sourcemap::Token =
            map.get_token(2).expect("third token exists");

        assert_eq!(
            b_source.get_source_id().and_then(|id: u32| map.get_source(id)),
            Some("b.ts"),
        );

        assert_eq!((b_source.get_src_line(), b_source.get_src_col()), (20, 5));
    }

    #[test]
    fn test_generate_code_keeps_input_map_name() {
        let options: ParseAstOptions = ParseAstOptions {
            cwd: PathBuf::new(),
            file: FILE.to_string(),
            language: None,
            source_type: None,
            input_map: Some(named_input_map()),
        };

        let ast: Ast =
            parse_ast("const a = 1;".to_string(), options).expect("parses");

        let result: CodeResult =
            generate_code(&ast, GenerateCodeOptions { file: FILE.to_string() });

        let map: SourceMap = result.map.expect("map always produced");

        assert_eq!(
            map.get_names().collect::<Vec<&str>>(),
            vec!["upstreamName"],
        );
        assert_eq!(
            map.get_token(0).and_then(|token: oxc_sourcemap::Token| {
                token.get_name_id()
            }),
            Some(0),
        );
    }

    #[test]
    fn test_generate_code_preserves_generated_name() {
        let options: ParseAstOptions = ParseAstOptions {
            cwd: PathBuf::new(),
            file: FILE.to_string(),
            language: None,
            source_type: None,
            input_map: Some(sample_input_map()),
        };

        let mut ast: Ast =
            parse_ast("const a = 1;".to_string(), options).expect("parses");

        ast.with_mut(|allocator: &Allocator, program: &mut Program<'_>| {
            let Statement::VariableDeclaration(declaration) =
                &mut program.body[0]
            else {
                panic!("expected a variable declaration");
            };

            let declarator: &mut VariableDeclarator<'_> = declaration
                .declarations
                .first_mut()
                .expect("declaration has a declarator");

            let BindingPattern::BindingIdentifier(identifier) =
                &mut declarator.id
            else {
                panic!("expected a binding identifier");
            };

            identifier.name = Ident::from_str_in("renamed", &allocator);
        });

        let result: CodeResult =
            generate_code(&ast, GenerateCodeOptions { file: FILE.to_string() });

        let map: SourceMap = result.map.expect("map always produced");

        assert_eq!(result.code, "const renamed = 1;");
        assert_eq!(map.get_names().collect::<Vec<&str>>(), vec!["a"]);
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

    #[test]
    fn test_generate_code_with_codegen_sets_single_quote() {
        let ast: Ast = parse_code("const a = \"hello\";");

        let result: CodeResult = generate_code_with_codegen(
            &ast,
            GenerateCodeOptions { file: FILE.to_string() },
            OxcCodegenOptions {
                single_quote: true,
                ..OxcCodegenOptions::default()
            },
        );

        assert_eq!(result.code, "const a = 'hello';");
    }

    #[test]
    fn test_generate_code_with_codegen_sets_ascii_only() {
        let ast: Ast = parse_code("const µ = 1;");

        let result: CodeResult = generate_code_with_codegen(
            &ast,
            GenerateCodeOptions { file: FILE.to_string() },
            OxcCodegenOptions {
                ascii_only: true,
                ..OxcCodegenOptions::default()
            },
        );

        assert_eq!(result.code, "const \\u00B5 = 1;");
    }

    #[test]
    fn test_generate_code_with_codegen_uses_space_indentation() {
        let ast: Ast = parse_code("function f() {\nreturn;\n}");

        let result: CodeResult = generate_code_with_codegen(
            &ast,
            GenerateCodeOptions { file: FILE.to_string() },
            OxcCodegenOptions {
                indent_char: oxc::codegen::IndentChar::Space,
                indent_width: 2,
                ..OxcCodegenOptions::default()
            },
        );

        assert_eq!(result.code, "function f() {\n  return;\n}");
    }

    #[test]
    fn test_generate_code_with_codegen_uses_tab_indentation() {
        let ast: Ast = parse_code("function f() {\nreturn;\n}");

        let result: CodeResult = generate_code_with_codegen(
            &ast,
            GenerateCodeOptions { file: FILE.to_string() },
            OxcCodegenOptions {
                indent_char: oxc::codegen::IndentChar::Tab,
                indent_width: 1,
                ..OxcCodegenOptions::default()
            },
        );

        assert_eq!(result.code, "function f() {\n\treturn;\n}");
    }

    #[test]
    fn test_generate_code_with_codegen_uses_indent_width() {
        let ast: Ast = parse_code("function f() {\nreturn;\n}");

        let result: CodeResult = generate_code_with_codegen(
            &ast,
            GenerateCodeOptions { file: FILE.to_string() },
            OxcCodegenOptions {
                indent_char: oxc::codegen::IndentChar::Space,
                indent_width: 4,
                ..OxcCodegenOptions::default()
            },
        );

        assert_eq!(result.code, "function f() {\n    return;\n}");
    }

    #[test]
    fn test_generate_code_with_codegen_prints_comments() {
        let ast: Ast = parse_code("// lead\nconst a = 1;");

        let result: CodeResult = generate_code_with_codegen(
            &ast,
            GenerateCodeOptions { file: FILE.to_string() },
            OxcCodegenOptions::default(),
        );

        assert!(result.code.contains("// lead"), "{}", result.code);
    }

    #[test]
    fn test_generate_code_with_codegen_disables_comments() {
        let ast: Ast = parse_code("// lead\nconst a = 1;");

        let result: CodeResult = generate_code_with_codegen(
            &ast,
            GenerateCodeOptions { file: FILE.to_string() },
            OxcCodegenOptions {
                comments: CommentOptions::disabled(),
                ..OxcCodegenOptions::default()
            },
        );

        assert!(!result.code.contains("// lead"), "{}", result.code);
    }

    #[test]
    fn test_generate_code_with_codegen_minifies_and_drops_comments() {
        let ast: Ast = parse_code("// lead\nconst a = 1;");

        let result: CodeResult = generate_code_with_codegen(
            &ast,
            GenerateCodeOptions { file: FILE.to_string() },
            OxcCodegenOptions::minify(),
        );

        assert_eq!(result.code, "const a=1;");
    }

    #[test]
    fn test_generate_code_with_codegen_explicit_comments_survive_minify() {
        let ast: Ast = parse_code("// lead\nconst a = 1;");

        let result: CodeResult = generate_code_with_codegen(
            &ast,
            GenerateCodeOptions { file: FILE.to_string() },
            OxcCodegenOptions {
                minify: true,
                comments: CommentOptions::default(),
                ..OxcCodegenOptions::default()
            },
        );

        assert!(result.code.contains("// lead"), "{}", result.code);
        assert!(!result.code.contains("const a = 1;"), "{}", result.code);
    }

    #[test]
    fn test_generate_code_with_codegen_ignores_source_map_path() {
        let ast: Ast = parse_code("const a = 1;");

        let result: CodeResult = generate_code_with_codegen(
            &ast,
            GenerateCodeOptions { file: FILE.to_string() },
            OxcCodegenOptions {
                source_map_path: Some(PathBuf::from("ignored.ts")),
                ..OxcCodegenOptions::default()
            },
        );

        let map: SourceMap = result.map.expect("map always produced");

        assert_eq!(map.get_file(), Some(FILE));
        assert_eq!(map.get_source(0), Some(FILE));
    }

    #[test]
    fn test_generate_program_uses_telarel_default_indentation() {
        let ast: Ast = parse_code("function f() {\nreturn;\n}");

        let (code, _): (String, SourceMap) =
            generate_program(ast.program(), FILE);

        assert_eq!(code, "function f() {\n  return;\n}\n");
    }

    #[test]
    fn test_generate_program_with_codegen_uses_supplied_options() {
        let ast: Ast = parse_code("const a = \"hello\";");

        let (code, _): (String, SourceMap) = generate_program_with_codegen(
            ast.program(),
            FILE,
            OxcCodegenOptions {
                single_quote: true,
                ..OxcCodegenOptions::default()
            },
        );

        assert_eq!(code, "const a = 'hello';\n");
    }

    #[test]
    fn test_generate_program_with_codegen_ignores_source_map_path() {
        let ast: Ast = parse_code("const a = 1;");

        let (_, map): (String, SourceMap) = generate_program_with_codegen(
            ast.program(),
            FILE,
            OxcCodegenOptions {
                source_map_path: Some(PathBuf::from("ignored.ts")),
                ..OxcCodegenOptions::default()
            },
        );

        assert_eq!(map.get_source(0), Some(FILE));
    }
}
