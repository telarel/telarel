use telarel_common::{AstResult, ParserOptions};

use crate::PluginError;

/// Arguments for the `parse` hook.
#[derive(Debug, Clone, Copy)]
pub struct ParseArgs<'a> {
    /// The code to parse.
    pub code: &'a str,
    /// The resolved parser options.
    pub options: &'a ParserOptions,
}

/// The `parse` hook return: a parsed AST, `None` (defer), or an error.
pub type ParseReturn = Result<Option<AstResult>, PluginError>;

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use telarel_common::{
        AstResult, Language, ParseAstOptions, ParserOptions, SourceMapOptions,
        SourceType, parse_ast,
    };

    use super::*;

    fn options() -> ParserOptions {
        ParserOptions {
            cwd: PathBuf::from("/repo"),
            file: String::from("a.ts"),
            language: Language::TS,
            source_type: SourceType::Module,
            source_map: SourceMapOptions::default(),
        }
    }

    #[test]
    fn test_parse_args_read_code_and_options() {
        let options: ParserOptions = options();

        let args: ParseArgs<'_> =
            ParseArgs { code: "let a;", options: &options };

        assert_eq!(args.code, "let a;");
        assert_eq!(args.options.file, "a.ts");
    }

    #[test]
    fn test_parse_return_defers_with_none() {
        let result: ParseReturn = Ok(None);

        assert!(matches!(result, Ok(None)));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_parse_return_carries_ast_result() {
        let ast = parse_ast(
            String::from("let a;"),
            ParseAstOptions {
                cwd: PathBuf::from("/repo"),
                file: String::from("a.ts"),
                language: None,
                source_type: None,
                input_map: None,
            },
        )
        .unwrap();

        let result: ParseReturn = Ok(Some(AstResult { ast }));

        assert!(matches!(result, Ok(Some(_))));
    }
}
