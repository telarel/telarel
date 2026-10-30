use crate::_types::options::builtin::generator::GeneratorOptions;
use crate::ast::ast::Ast;
use crate::ast::generate::{GenerateCodeOptions, generate_code};
use crate::ast::result::CodeResult;
use crate::errors::compile::CompileError;
use crate::sourcemap::resolve::attach_input_map;

/// A builtin generator over a borrowed [`Ast`].
///
/// The AST is borrowed because hook arguments are read-only.
#[derive(Debug, Clone)]
pub struct Generator<'a> {
    ast: &'a Ast,
    options: GeneratorOptions,
}

impl<'a> Generator<'a> {
    /// Create a generator over `ast`.
    pub fn new(
        ast: &'a Ast,
        options: GeneratorOptions,
    ) -> Self {
        Self { ast, options }
    }

    /// Generate the composed output: code plus the source map.
    ///
    /// An explicit input request wins when it resolves; a best-effort miss
    /// keeps the AST's own map. Output delivery is not applied here; the
    /// pipeline owns it.
    pub fn generate(self) -> Result<CodeResult, CompileError> {
        let ast: Ast = attach_input_map(
            self.ast,
            &self.options.cwd,
            &self.options.file,
            self.options.source_map.input.as_ref(),
        )?;

        let result: CodeResult = generate_code(
            &ast,
            GenerateCodeOptions { file: self.options.file },
        );

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use oxc_sourcemap::SourceMapBuilder;
    use tempfile::TempDir;

    use crate::_types::options::sourcemap::SourceMapOptions;
    use crate::_types::options::sourcemap::input::{
        InputSourceMap, InputSourceMapMode,
    };
    use crate::_types::options::sourcemap::output::{
        OutputSourceMap, OutputSourceMapMode,
    };
    use crate::ast::parse::{ParseAstOptions, parse_ast};
    use crate::helpers::test_util::{path_buf, temp_dir, write_file};
    use crate::sourcemap::SourceMap;
    use crate::sourcemap::inline::append_inline_source_map;

    use super::*;

    const FILE: &str = "index.ts";

    fn map_to(
        file: &str,
        line: u32,
    ) -> SourceMap {
        let mut builder: SourceMapBuilder<'_> = SourceMapBuilder::default();

        builder.set_file(file);

        let source_id: u32 =
            builder.add_source_and_content(file, "const a = 1;");

        builder.add_token(0, 0, line, 0, Some(source_id), None);

        builder.into_owned_sourcemap().into_inner()
    }

    fn input_map() -> SourceMap {
        // original line 9 col 0 produced into parse-input (0, 0)
        map_to("upstream.ts", 9)
    }

    fn parse_code_with_input_map(
        code: &str,
        map: Option<SourceMap>,
    ) -> Ast {
        let options: ParseAstOptions = ParseAstOptions {
            cwd: PathBuf::new(),
            file: FILE.to_string(),
            language: None,
            source_type: None,
            input_map: map,
        };

        parse_ast(code.to_string(), options).expect("parses")
    }

    fn parse_with_input_map(map: Option<SourceMap>) -> Ast {
        parse_code_with_input_map("const a = 1;", map)
    }

    fn assert_same_map(
        actual: &SourceMap,
        expected: &SourceMap,
    ) {
        assert_eq!(actual.to_json_string(), expected.to_json_string());
    }

    fn options() -> GeneratorOptions {
        GeneratorOptions {
            cwd: PathBuf::new(),
            file: FILE.to_string(),
            source_map: SourceMapOptions::default(),
        }
    }

    #[test]
    fn test_generate_bare_uses_codegen_map() {
        let ast: Ast = parse_with_input_map(None);

        let result: CodeResult =
            Generator::new(&ast, options()).generate().expect("generates");

        let map: SourceMap = result.map.expect("map always produced");
        let token: oxc_sourcemap::Token =
            map.get_token(0).expect("first token exists");

        assert_eq!(token.get_dst_line(), 0);
        assert_eq!(token.get_dst_col(), 0);
        assert_eq!(token.get_src_line(), 0);
        assert_eq!(token.get_src_col(), 0);
    }

    #[test]
    fn test_generate_with_input_map_composes_positions() {
        let ast: Ast = parse_with_input_map(Some(input_map()));

        let result: CodeResult =
            Generator::new(&ast, options()).generate().expect("generates");

        let map: SourceMap = result.map.expect("map always produced");
        let token: oxc_sourcemap::Token =
            map.get_token(0).expect("first token exists");

        // Generated (0, 0) maps back to the upstream original (9, 0).
        assert_eq!(token.get_dst_line(), 0);
        assert_eq!(token.get_dst_col(), 0);
        assert_eq!(token.get_src_line(), 9);
        assert_eq!(token.get_src_col(), 0);

        assert_eq!(map.get_source(0), Some("upstream.ts"));
        assert_eq!(map.get_source_content(0), Some("const a = 1;"));
    }

    #[test]
    fn test_generate_strips_trailing_newline() {
        let ast: Ast = parse_with_input_map(None);

        let result: CodeResult =
            Generator::new(&ast, options()).generate().expect("generates");

        assert_eq!(result.code, "const a = 1;");
    }

    #[test]
    fn test_generate_value_request_overrides_ast_map() {
        let ast: Ast = parse_with_input_map(Some(input_map()));
        let replacement: SourceMap = map_to("replacement.ts", 4);

        let mut options: GeneratorOptions = options();
        options.source_map.input =
            Some(InputSourceMap::Mode(InputSourceMapMode::Value(replacement)));

        let result: CodeResult =
            Generator::new(&ast, options).generate().expect("generates");

        let map: SourceMap = result.map.expect("map always produced");
        let token: oxc_sourcemap::Token =
            map.get_token(0).expect("first token exists");

        assert_eq!(token.get_src_line(), 4);
        assert_eq!(token.get_src_col(), 0);
        assert_eq!(map.get_source(0), Some("replacement.ts"));
    }

    #[test]
    fn test_generate_inline_request_attaches_map_and_strips_consumed_comment() {
        let inline_map: SourceMap = map_to("inline.ts", 3);
        let code: String =
            append_inline_source_map(String::from("const a = 1;"), &inline_map);
        let ast: Ast = parse_code_with_input_map(&code, Some(input_map()));

        let mut options: GeneratorOptions = options();
        options.source_map.input =
            Some(InputSourceMap::Mode(InputSourceMapMode::Inline));

        // `CodeResult` does not expose the AST that `generate` builds, so
        // verify the shared attachment path directly.
        let attached: Ast = attach_input_map(
            &ast,
            &options.cwd,
            &options.file,
            options.source_map.input.as_ref(),
        )
        .expect("inline request resolves");

        assert_same_map(attached.map().expect("map attached"), &inline_map);
        assert!(attached.program().comments.is_empty());

        let result: CodeResult =
            Generator::new(&ast, options).generate().expect("generates");

        let map: SourceMap = result.map.expect("map always produced");
        let token: oxc_sourcemap::Token =
            map.get_token(0).expect("first token exists");

        assert_eq!(token.get_src_line(), 3);
        assert_eq!(map.get_source(0), Some("inline.ts"));
    }

    #[test]
    fn test_generate_external_request_reads_map_through_cwd() {
        let dir: TempDir = temp_dir();
        let external_map: SourceMap = map_to("external.ts", 7);

        write_file(&dir, "index.ts.map", &external_map.to_json_string());

        let ast: Ast = parse_with_input_map(None);

        let mut options: GeneratorOptions = options();
        options.cwd = path_buf(&dir);
        options.source_map.input =
            Some(InputSourceMap::Mode(InputSourceMapMode::External));

        let result: CodeResult =
            Generator::new(&ast, options).generate().expect("generates");

        let map: SourceMap = result.map.expect("map always produced");
        let token: oxc_sourcemap::Token =
            map.get_token(0).expect("first token exists");

        assert_eq!(token.get_src_line(), 7);
        assert_eq!(map.get_source(0), Some("external.ts"));
    }

    #[test]
    fn test_generate_auto_request_prefers_inline_then_external() {
        let dir: TempDir = temp_dir();
        let inline_map: SourceMap = map_to("inline.ts", 2);
        let external_map: SourceMap = map_to("external.ts", 6);

        write_file(&dir, "index.ts.map", &external_map.to_json_string());

        let code: String =
            append_inline_source_map(String::from("const a = 1;"), &inline_map);
        let inline_ast: Ast = parse_code_with_input_map(&code, None);

        let mut inline_options: GeneratorOptions = options();
        inline_options.cwd = path_buf(&dir);
        inline_options.source_map.input =
            Some(InputSourceMap::Mode(InputSourceMapMode::Auto));

        let inline_result: CodeResult =
            Generator::new(&inline_ast, inline_options)
                .generate()
                .expect("generates");

        let inline_output: SourceMap =
            inline_result.map.expect("map always produced");
        let inline_token: oxc_sourcemap::Token =
            inline_output.get_token(0).expect("first token exists");

        assert_eq!(inline_token.get_src_line(), 2);
        assert_eq!(inline_output.get_source(0), Some("inline.ts"));

        let external_ast: Ast = parse_with_input_map(None);

        let mut external_options: GeneratorOptions = options();
        external_options.cwd = path_buf(&dir);
        external_options.source_map.input =
            Some(InputSourceMap::Mode(InputSourceMapMode::Auto));

        let external_result: CodeResult =
            Generator::new(&external_ast, external_options)
                .generate()
                .expect("generates");

        let external_output: SourceMap =
            external_result.map.expect("map always produced");
        let external_token: oxc_sourcemap::Token =
            external_output.get_token(0).expect("first token exists");

        assert_eq!(external_token.get_src_line(), 6);
        assert_eq!(external_output.get_source(0), Some("external.ts"));
    }

    #[test]
    fn test_generate_best_effort_miss_keeps_ast_map() {
        let dir: TempDir = temp_dir();
        let ast: Ast = parse_with_input_map(Some(input_map()));

        for mode in [InputSourceMapMode::Inline, InputSourceMapMode::Auto] {
            let mut options: GeneratorOptions = options();
            options.cwd = path_buf(&dir);
            options.source_map.input = Some(InputSourceMap::Mode(mode));

            let result: CodeResult =
                Generator::new(&ast, options).generate().expect("generates");

            let map: SourceMap = result.map.expect("map always produced");
            let token: oxc_sourcemap::Token =
                map.get_token(0).expect("first token exists");

            assert_eq!(token.get_src_line(), 9);
            assert_eq!(map.get_source(0), Some("upstream.ts"));
        }
    }

    #[test]
    fn test_generate_without_input_request_composes_ast_map() {
        let ast: Ast = parse_with_input_map(Some(input_map()));

        let mut options: GeneratorOptions = options();
        options.source_map.output =
            Some(OutputSourceMap::Mode(OutputSourceMapMode::Inline));

        let result: CodeResult =
            Generator::new(&ast, options).generate().expect("generates");

        let map: SourceMap = result.map.expect("map always produced");
        let token: oxc_sourcemap::Token =
            map.get_token(0).expect("first token exists");

        assert_eq!(token.get_src_line(), 9);
        assert_eq!(token.get_src_col(), 0);
        assert_eq!(map.get_source(0), Some("upstream.ts"));
        assert!(!result.code.contains("sourceMappingURL"));
    }
}
