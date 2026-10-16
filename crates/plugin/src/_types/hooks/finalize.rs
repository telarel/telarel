use telarel_common::Ast;

/// Arguments for the `finalize` hook: the final AST, read-only.
///
/// `finalize` is an observer; its return value is ignored.
#[derive(Debug, Clone, Copy)]
pub struct FinalizeArgs<'a> {
    /// The final AST, read-only.
    pub ast: &'a Ast,
}

#[cfg(test)]
mod tests {
    use telarel_common::{Ast, ParseAstOptions, parse_ast};

    use super::*;

    #[test]
    fn test_finalize_args_borrow_ast() {
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

        let args: FinalizeArgs<'_> = FinalizeArgs { ast: &ast };

        assert_eq!(args.ast.source().as_ref(), "let a;");
    }
}
