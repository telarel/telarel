use crate::_types::options::builtin::parser::ParserOptions;
use crate::ast::parse::{ParseAstOptions, parse_ast};
use crate::ast::result::AstResult;
use crate::errors::compile::CompileError;
use crate::sourcemap::SourceMap;
use crate::sourcemap::resolve::resolve_input_source_map;

/// A builtin parser that resolves its input source map, parses the code, and
/// attaches the map to the resulting [`AstResult`].
#[derive(Debug, Clone)]
pub struct Parser {
    code: String,
    options: ParserOptions,
}

impl Parser {
    /// Create a parser over `code`.
    pub fn new(
        code: String,
        options: ParserOptions,
    ) -> Self {
        Self { code, options }
    }

    /// Resolve the input source map (per `options.source_map.input`), parse
    /// the code, and attach the map to the returned AST.
    pub fn parse(self) -> Result<AstResult, CompileError> {
        let input_map: Option<SourceMap> = match &self.options.source_map.input
        {
            | Some(input) => resolve_input_source_map(
                &self.code,
                &self.options.cwd,
                &self.options.file,
                input,
            )?,
            | None => None,
        };

        let ast = parse_ast(
            self.code,
            ParseAstOptions {
                cwd: self.options.cwd.clone(),
                file: self.options.file.clone(),
                language: Some(self.options.language),
                source_type: Some(self.options.source_type),
                input_map,
            },
        )?;

        Ok(AstResult { ast })
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use oxc_sourcemap::SourceMapBuilder;
    use tempfile::TempDir;

    use crate::_types::grammar::language::Language;
    use crate::_types::grammar::source_type::SourceType;
    use crate::_types::options::source_map::SourceMapOptions;
    use crate::_types::options::source_map::input::{
        InputSourceMap, InputSourceMapMode,
    };
    use crate::helpers::test_util::{path_string, temp_dir, write_file};
    use crate::sourcemap::inline::append_inline_source_map;

    use super::*;

    fn sample_map(file: &str) -> SourceMap {
        let mut builder: SourceMapBuilder<'_> = SourceMapBuilder::default();

        builder.set_file(file);

        let source_id: u32 =
            builder.add_source_and_content(file, "const a = 1;");

        builder.add_token(0, 0, 9, 0, Some(source_id), None);

        builder.into_owned_sourcemap().into_inner()
    }

    fn source_map_options(input: InputSourceMap) -> SourceMapOptions {
        SourceMapOptions { input: Some(input), output: None }
    }

    fn options(
        cwd: &str,
        file: &str,
        source_map: SourceMapOptions,
    ) -> ParserOptions {
        ParserOptions {
            cwd: cwd.to_string(),
            file: file.to_string(),
            language: Language::TS,
            source_type: SourceType::Unambiguous,
            source_map,
        }
    }

    fn assert_same_map(
        actual: &SourceMap,
        expected: &SourceMap,
    ) {
        assert_eq!(actual.to_json_string(), expected.to_json_string());
    }

    #[test]
    fn test_parse_without_source_map_attaches_none() {
        let result: AstResult = Parser::new(
            String::from("const a = 1;"),
            options("", "index.ts", SourceMapOptions::default()),
        )
        .parse()
        .expect("valid source parses");

        assert!(result.ast.map().is_none());
    }

    #[test]
    fn test_parse_respects_concrete_language_and_source_type() {
        let parser: Parser = Parser::new(
            String::from("const a: number = 1;"),
            ParserOptions {
                cwd: String::new(),
                file: String::from("app.js"),
                language: Language::TS,
                source_type: SourceType::Script,
                source_map: SourceMapOptions::default(),
            },
        );

        let result: AstResult = parser.parse().expect("explicit options parse");

        assert!(result.ast.program().source_type.is_typescript());
        assert!(result.ast.program().source_type.is_script());
    }

    #[test]
    fn test_parse_propagates_invalid_syntax() {
        let result: Result<AstResult, CompileError> = Parser::new(
            String::from("const = ;"),
            options("", "index.ts", SourceMapOptions::default()),
        )
        .parse();

        let error: CompileError = result.expect_err("invalid syntax errors");

        assert!(error.to_string().contains("index.ts"));
    }

    #[test]
    fn test_parse_value_mode_attaches_map() {
        let map: SourceMap = sample_map("upstream.ts");

        let result: AstResult = Parser::new(
            String::from("const a = 1;"),
            options(
                "",
                "index.ts",
                source_map_options(InputSourceMap::Mode(
                    InputSourceMapMode::Value(map.clone()),
                )),
            ),
        )
        .parse()
        .expect("value mode parses");

        assert_same_map(result.ast.map().expect("map attached"), &map);
    }

    #[test]
    fn test_parse_inline_mode_attaches_map() {
        let map: SourceMap = sample_map("upstream.ts");

        let code: String =
            append_inline_source_map(String::from("const a = 1;"), &map);

        let result: AstResult = Parser::new(
            code,
            options(
                "",
                "index.ts",
                source_map_options(InputSourceMap::Mode(
                    InputSourceMapMode::Inline,
                )),
            ),
        )
        .parse()
        .expect("inline mode parses");

        assert_same_map(result.ast.map().expect("map attached"), &map);
    }

    #[test]
    fn test_parse_path_mode_attaches_map() {
        let dir: TempDir = temp_dir();
        let map: SourceMap = sample_map("upstream.ts");

        write_file(&dir, "custom.map", &map.to_json_string());

        let result: AstResult = Parser::new(
            String::from("const a = 1;"),
            options(
                &path_string(&dir),
                "index.ts",
                source_map_options(InputSourceMap::Mode(
                    InputSourceMapMode::Path(PathBuf::from("custom.map")),
                )),
            ),
        )
        .parse()
        .expect("path mode parses");

        assert_same_map(result.ast.map().expect("map attached"), &map);
    }

    #[test]
    fn test_parse_external_mode_attaches_map() {
        let dir: TempDir = temp_dir();
        let map: SourceMap = sample_map("upstream.ts");

        write_file(&dir, "index.ts.map", &map.to_json_string());

        let result: AstResult = Parser::new(
            String::from("const a = 1;"),
            options(
                &path_string(&dir),
                "index.ts",
                source_map_options(InputSourceMap::Mode(
                    InputSourceMapMode::External,
                )),
            ),
        )
        .parse()
        .expect("external mode parses");

        assert_same_map(result.ast.map().expect("map attached"), &map);
    }

    #[test]
    fn test_parse_auto_mode_inline_wins() {
        let dir: TempDir = temp_dir();
        let inline_map: SourceMap = sample_map("inline.ts");
        let external_map: SourceMap = sample_map("external.ts");

        write_file(&dir, "index.ts.map", &external_map.to_json_string());

        let code: String =
            append_inline_source_map(String::from("const a = 1;"), &inline_map);

        let result: AstResult = Parser::new(
            code,
            options(
                &path_string(&dir),
                "index.ts",
                source_map_options(InputSourceMap::Mode(
                    InputSourceMapMode::Auto,
                )),
            ),
        )
        .parse()
        .expect("auto mode parses");

        assert_same_map(result.ast.map().expect("map attached"), &inline_map);
    }

    #[test]
    fn test_parse_auto_mode_missing_attaches_none() {
        let dir: TempDir = temp_dir();

        let result: AstResult = Parser::new(
            String::from("const a = 1;"),
            options(
                &path_string(&dir),
                "index.ts",
                source_map_options(InputSourceMap::Mode(
                    InputSourceMapMode::Auto,
                )),
            ),
        )
        .parse()
        .expect("auto mode is best effort");

        assert!(result.ast.map().is_none());
    }
}
