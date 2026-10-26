use telarel_common::Ast;

use crate::PluginError;

/// Arguments for the `transform` hook.
///
/// The AST is an owned cheap handle. A plugin that changes the tree mutates it
/// through [`Ast::with_mut`] and returns it in [`TransformOutput`]. A plugin
/// that changes the source text must build a new [`Ast`] instead. Replacing
/// `Program::source_text` in place is unsupported.
#[derive(Clone)]
pub struct TransformArgs {
    /// The current AST. The driver keeps its own handle, so mutation copies on
    /// write. Returning it as [`TransformOutput`] replaces the carried AST.
    pub ast: Ast,
}

/// Result of the `transform` hook: the AST that replaces the carried one.
///
/// Returning `None` leaves the carried AST untouched. Returning `Some`
/// replaces it for the rest of the chain.
#[derive(Debug)]
pub struct TransformOutput {
    /// The replacement AST.
    pub ast: Ast,
}

/// The `transform` hook return: a replacement owned AST, `None`, or an error.
pub type TransformReturn = Result<Option<TransformOutput>, PluginError>;

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use telarel_common::{Ast, ParseAstOptions, parse_ast};

    use super::*;

    #[test]
    fn test_transform_output_wraps_program() {
        let ast: Ast = parse_ast(
            String::from("let a;"),
            ParseAstOptions {
                cwd: PathBuf::from(""),
                file: String::from("a.ts"),
                language: None,
                source_type: None,
                input_map: None,
            },
        )
        .unwrap();

        let output: TransformOutput = TransformOutput { ast };

        assert_eq!(output.ast.program().span.start, 0);
    }
}
