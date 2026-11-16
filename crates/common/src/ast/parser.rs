use crate::_types::options::OxcParseOptions;
use crate::_types::options::builtin::parser::ResolvedParserOptions;
use crate::ast::ast::Ast;
use crate::ast::parse::{ParseAstOptions, parse_ast_with_parser_options};
use crate::ast::result::AstResult;
use crate::errors::compile::CompileError;
use crate::helpers::resolve::apply_curated_parser_options;
use crate::sourcemap::SourceMap;
use crate::sourcemap::inline::strip_consumed_inline_source_map_comment;
use crate::sourcemap::resolve::resolve_input_source_map;

/// A builtin parser that attaches the requested input source map to its result.
#[derive(Debug, Clone)]
pub struct Parser {
    code: String,
    options: ResolvedParserOptions,
}

impl Parser {
    /// Create a parser over `code`.
    pub fn new(
        code: String,
        options: ResolvedParserOptions,
    ) -> Self {
        Self { code, options }
    }

    /// Parse the code and return the AST with the requested input map attached.
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

        let mut parser_options: OxcParseOptions = self.options.oxc;

        apply_curated_parser_options(
            &mut parser_options,
            Some(self.options.preserve_parens),
            Some(self.options.allow_return_outside_function),
        );

        let mut ast: Ast = parse_ast_with_parser_options(
            self.code,
            ParseAstOptions {
                cwd: self.options.cwd.clone(),
                file: self.options.file.clone(),
                language: Some(self.options.language),
                source_type: Some(self.options.source_type),
                input_map,
            },
            parser_options,
        )?;

        if let Some(input) = self.options.source_map.input.as_ref() {
            strip_consumed_inline_source_map_comment(&mut ast, input);
        }

        Ok(AstResult { ast })
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use oxc::ast::ast::{Comment, Expression, Statement};
    use oxc_sourcemap::SourceMapBuilder;
    use tempfile::TempDir;

    use crate::_types::grammar::language::Language;
    use crate::_types::grammar::source_type::SourceType;
    use crate::_types::options::sourcemap::SourceMapOptions;
    use crate::_types::options::sourcemap::input::{
        InputSourceMap, InputSourceMapMode, InputSourceMapOptions,
    };
    use crate::ast::ast::Ast;
    use crate::helpers::resolve::default_parser_oxc;
    use crate::helpers::test_util::{
        comment_text, first_initializer, path_buf, temp_dir, write_file,
    };
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
        cwd: &Path,
        file: &str,
        source_map: SourceMapOptions,
    ) -> ResolvedParserOptions {
        ResolvedParserOptions {
            cwd: cwd.to_path_buf(),
            file: file.to_string(),
            language: Language::TS,
            source_type: SourceType::Unambiguous,
            source_map,
            preserve_parens: false,
            allow_return_outside_function: false,
            oxc: default_parser_oxc(),
        }
    }

    fn assert_same_map(
        actual: &SourceMap,
        expected: &SourceMap,
    ) {
        assert_eq!(actual.to_json_string(), expected.to_json_string());
    }

    fn has_source_mapping_comment(ast: &Ast) -> bool {
        ast.program().comments.iter().any(|comment: &Comment| {
            comment_text(ast, comment).contains("sourceMappingURL")
        })
    }

    #[test]
    fn test_parse_without_source_map_attaches_none() {
        let result: AstResult = Parser::new(
            String::from("const a = 1;"),
            options(Path::new(""), "index.ts", SourceMapOptions::default()),
        )
        .parse()
        .expect("valid source parses");

        assert!(result.ast.map().is_none());
    }

    #[test]
    fn test_parse_respects_concrete_language_and_source_type() {
        let parser: Parser = Parser::new(
            String::from("const a: number = 1;"),
            ResolvedParserOptions {
                cwd: PathBuf::new(),
                file: String::from("app.js"),
                language: Language::TS,
                source_type: SourceType::Script,
                source_map: SourceMapOptions::default(),
                preserve_parens: false,
                allow_return_outside_function: false,
                oxc: default_parser_oxc(),
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
            options(Path::new(""), "index.ts", SourceMapOptions::default()),
        )
        .parse();

        let error: CompileError = result.expect_err("invalid syntax errors");

        assert!(error.to_string().contains("index.ts"));
    }

    #[test]
    fn test_parse_preserve_parens_false_removes_parenthesized_expression() {
        let result: AstResult = Parser::new(
            String::from("const a = (1);"),
            options(Path::new(""), "index.ts", SourceMapOptions::default()),
        )
        .parse()
        .expect("valid source parses");

        assert!(matches!(
            first_initializer(&result.ast),
            Expression::NumericLiteral(_),
        ));
    }

    #[test]
    fn test_parse_preserve_parens_true_keeps_parenthesized_expression() {
        let mut parser_options: ResolvedParserOptions =
            options(Path::new(""), "index.ts", SourceMapOptions::default());

        parser_options.preserve_parens = true;

        let result: AstResult =
            Parser::new(String::from("const a = (1);"), parser_options)
                .parse()
                .expect("valid source parses");

        assert!(matches!(
            first_initializer(&result.ast),
            Expression::ParenthesizedExpression(_),
        ));
    }

    #[test]
    fn test_parse_rejects_top_level_return_by_default() {
        let result: Result<AstResult, CompileError> = Parser::new(
            String::from("return;"),
            options(Path::new(""), "index.ts", SourceMapOptions::default()),
        )
        .parse();

        assert!(result.is_err());
    }

    #[test]
    fn test_parse_allow_return_outside_function_accepts_top_level_return() {
        let mut parser_options: ResolvedParserOptions =
            options(Path::new(""), "index.ts", SourceMapOptions::default());

        parser_options.allow_return_outside_function = true;

        let result: AstResult =
            Parser::new(String::from("return;"), parser_options)
                .parse()
                .expect("top-level return is accepted");

        assert!(matches!(
            &result.ast.program().body[0],
            Statement::ReturnStatement(_),
        ));
    }

    #[test]
    fn test_parse_uses_raw_oxc_parser_options() {
        let mut parser_options: ResolvedParserOptions =
            options(Path::new(""), "index.ts", SourceMapOptions::default());

        parser_options.oxc.allow_v8_intrinsics = true;

        let result: AstResult =
            Parser::new(String::from("%DebugPrint('x')"), parser_options)
                .parse()
                .expect("v8 intrinsic parses when enabled");

        let Statement::ExpressionStatement(statement) =
            &result.ast.program().body[0]
        else {
            panic!("expected an expression statement");
        };

        assert!(matches!(
            &statement.expression,
            Expression::V8IntrinsicExpression(_),
        ));
    }

    #[test]
    fn test_parse_value_mode_attaches_map() {
        let map: SourceMap = sample_map("upstream.ts");

        let result: AstResult = Parser::new(
            String::from("const a = 1;"),
            options(
                Path::new(""),
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
    fn test_parse_value_mode_keeps_inline_comment() {
        let map: SourceMap = sample_map("upstream.ts");

        let code: String =
            append_inline_source_map(String::from("const a = 1;"), &map);

        let result: AstResult = Parser::new(
            code,
            options(
                Path::new(""),
                "index.ts",
                source_map_options(InputSourceMap::Mode(
                    InputSourceMapMode::Value(map.clone()),
                )),
            ),
        )
        .parse()
        .expect("value mode parses");

        assert_same_map(result.ast.map().expect("map attached"), &map);
        assert!(has_source_mapping_comment(&result.ast));
    }

    #[test]
    fn test_parse_inline_mode_attaches_map() {
        let map: SourceMap = sample_map("upstream.ts");

        let code: String =
            append_inline_source_map(String::from("const a = 1;"), &map);

        let result: AstResult = Parser::new(
            code,
            options(
                Path::new(""),
                "index.ts",
                source_map_options(InputSourceMap::Mode(
                    InputSourceMapMode::Inline,
                )),
            ),
        )
        .parse()
        .expect("inline mode parses");

        assert_same_map(result.ast.map().expect("map attached"), &map);
        assert!(result.ast.source().contains("sourceMappingURL"));
        assert!(!has_source_mapping_comment(&result.ast));
    }

    #[test]
    fn test_parse_external_mode_with_path_attaches_map() {
        let dir: TempDir = temp_dir();
        let map: SourceMap = sample_map("upstream.ts");

        write_file(&dir, "custom.map", &map.to_json_string());

        let result: AstResult = Parser::new(
            String::from("const a = 1;"),
            options(
                &path_buf(&dir),
                "index.ts",
                source_map_options(InputSourceMap::Options(
                    InputSourceMapOptions {
                        mode: InputSourceMapMode::External,
                        path: Some(PathBuf::from("custom.map")),
                    },
                )),
            ),
        )
        .parse()
        .expect("external path parses");

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
                &path_buf(&dir),
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
                &path_buf(&dir),
                "index.ts",
                source_map_options(InputSourceMap::Mode(
                    InputSourceMapMode::Auto,
                )),
            ),
        )
        .parse()
        .expect("auto mode parses");

        assert_same_map(result.ast.map().expect("map attached"), &inline_map);
        assert!(result.ast.source().contains("sourceMappingURL"));
        assert!(!has_source_mapping_comment(&result.ast));
    }

    #[test]
    fn test_parse_auto_mode_missing_attaches_none() {
        let dir: TempDir = temp_dir();

        let result: AstResult = Parser::new(
            String::from("const a = 1;"),
            options(
                &path_buf(&dir),
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
