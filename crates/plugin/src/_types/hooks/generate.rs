use telarel_common::{Ast, CodeResult, GeneratorOptions};

use crate::PluginError;

/// Arguments for the `generate` hook.
#[derive(Debug, Clone, Copy)]
pub struct GenerateArgs<'a> {
    /// The AST to generate from, read-only.
    pub ast: &'a Ast,
    /// The resolved generator options.
    pub options: &'a GeneratorOptions,
}

/// The `generate` hook return: generated code, `None` (defer), or an error.
pub type GenerateReturn = Result<Option<CodeResult>, PluginError>;

#[cfg(test)]
mod tests {
    use telarel_common::{
        CodeResult, GeneratorOptions, ParseAstOptions, SourceMapOptions,
        parse_ast,
    };

    use super::*;

    fn options() -> GeneratorOptions {
        GeneratorOptions {
            cwd: String::from("/repo"),
            file: String::from("a.ts"),
            source_map: SourceMapOptions::default(),
        }
    }

    #[test]
    fn test_generate_args_read_ast_and_options() {
        let ast: Ast = parse_ast(
            String::from("let a;"),
            ParseAstOptions {
                cwd: String::from("/repo"),
                file: String::from("a.ts"),
                language: None,
                source_type: None,
                input_map: None,
            },
        )
        .unwrap();

        let options: GeneratorOptions = options();

        let args: GenerateArgs<'_> =
            GenerateArgs { ast: &ast, options: &options };

        assert_eq!(args.ast.source().as_ref(), "let a;");
        assert_eq!(args.options.file, "a.ts");
    }

    #[test]
    fn test_generate_return_defers_with_none() {
        let result: GenerateReturn = Ok(None);

        assert!(matches!(result, Ok(None)));
    }

    #[test]
    fn test_generate_return_carries_code_result() {
        let result: GenerateReturn =
            Ok(Some(CodeResult { code: String::from("let a;"), map: None }));

        if let Ok(Some(output)) = result {
            assert_eq!(output.code, "let a;");
        } else {
            panic!("expected Ok(Some(CodeResult))");
        }
    }
}
