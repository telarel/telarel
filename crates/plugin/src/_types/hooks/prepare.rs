use telarel_common::Ast;

/// Arguments for the `prepare` hook: the parsed AST, read-only.
///
/// `prepare` is an observer; its return value is ignored.
#[derive(Debug, Clone, Copy)]
pub struct PrepareArgs<'a> {
    /// The parsed AST, read-only.
    pub ast: &'a Ast,
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use telarel_common::{Ast, ParseAstOptions, parse_ast};

    use super::*;

    #[test]
    fn test_prepare_args_borrow_ast() {
        let ast: Ast = parse_ast(
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

        let args: PrepareArgs<'_> = PrepareArgs { ast: &ast };

        assert_eq!(args.ast.source().as_ref(), "let a;");
    }
}
