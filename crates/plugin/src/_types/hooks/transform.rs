use telarel_common::Ast;

/// Arguments for the `transform` hook.
///
/// The AST is read-only. A plugin that changes the tree builds a new owned
/// [`Ast`] and returns it in [`TransformOutput`].
#[derive(Clone, Copy)]
pub struct TransformArgs<'a> {
    /// The current AST, read-only. A plugin that changes the tree must derive
    /// a new owned AST and return it as [`TransformOutput`].
    pub ast: &'a Ast,
}

/// Result of the `transform` hook: the AST that replaces the carried one.
///
/// Returning `None` leaves the carried AST untouched; returning `Some`
/// replaces it for the rest of the chain.
#[derive(Debug)]
pub struct TransformOutput {
    /// The replacement AST.
    pub ast: Ast,
}

/// The `transform` hook return: a replacement owned AST, `None`, or an error.
pub type TransformReturn = anyhow::Result<Option<TransformOutput>>;

#[cfg(test)]
mod tests {
    use telarel_common::{Ast, CompileContext, ParseOwnedOptions, parse_owned};

    use super::*;

    #[test]
    fn test_transform_output_wraps_program() {
        let ctx: CompileContext<'_> = CompileContext::new("", "a.ts", "let a;");

        let ast: Ast = parse_owned(ParseOwnedOptions {
            context: &ctx,
            file: "a.ts",
            code: "let a;",
            language: None,
            source_type: None,
        })
        .unwrap();

        let output: TransformOutput = TransformOutput { ast };

        assert_eq!(output.ast.program().span.start, 0);
    }
}
