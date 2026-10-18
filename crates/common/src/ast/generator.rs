use crate::_types::options::builtin::generator::GeneratorOptions;
use crate::ast::ast::Ast;
use crate::ast::generate::{GenerateCodeOptions, generate_code};
use crate::ast::result::CodeResult;
use crate::errors::compile::CompileError;

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

    /// Generate external-form output: code plus the composed source map.
    ///
    /// Output-mode post-processing (inline/`both` comments) is applied by the
    /// pipeline, not here. Returns `Ok` today; the `Result` is kept for future
    /// error paths.
    pub fn generate(self) -> Result<CodeResult, CompileError> {
        let result: CodeResult = generate_code(
            self.ast,
            GenerateCodeOptions { file: self.options.file },
        );

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use oxc_sourcemap::SourceMapBuilder;

    use crate::_types::options::source_map::SourceMapOptions;
    use crate::ast::parse::{ParseAstOptions, parse_ast};
    use crate::sourcemap::SourceMap;

    use super::*;

    const FILE: &str = "index.ts";

    fn input_map() -> SourceMap {
        let mut builder: SourceMapBuilder<'_> = SourceMapBuilder::default();

        builder.set_file("upstream.ts");

        let source_id: u32 =
            builder.add_source_and_content("upstream.ts", "const a = 1;");

        // original line 9 col 0 produced into parse-input (0, 0)
        builder.add_token(0, 0, 9, 0, Some(source_id), None);

        builder.into_owned_sourcemap().into_inner()
    }

    fn parse_with_input_map(map: Option<SourceMap>) -> Ast {
        let options: ParseAstOptions = ParseAstOptions {
            cwd: String::new(),
            file: FILE.to_string(),
            language: None,
            source_type: None,
            input_map: map,
        };

        parse_ast(String::from("const a = 1;"), options).expect("parses")
    }

    fn options() -> GeneratorOptions {
        GeneratorOptions {
            cwd: String::new(),
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
    }

    #[test]
    fn test_generate_strips_trailing_newline() {
        let ast: Ast = parse_with_input_map(None);

        let result: CodeResult =
            Generator::new(&ast, options()).generate().expect("generates");

        assert_eq!(result.code, "const a = 1;");
    }
}
