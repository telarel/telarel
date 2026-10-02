use oxc::allocator::Allocator;
use oxc::ast::ast::Program;

/// Arguments for the `transform` hook.
///
/// `'ast` is the lifetime of the AST inside the compile allocator. The AST
/// is read-only; a plugin that changes the tree builds a new program rooted
/// in `allocator` and returns it in [`TransformOutput`].
#[derive(Clone, Copy)]
pub struct TransformArgs<'ast> {
    /// The allocator the AST is rooted in; interned strings and new nodes
    /// must be allocated here.
    pub allocator: &'ast Allocator,
    /// The current AST, read-only. A plugin that changes the tree must derive
    /// a new program rooted in `allocator` and return it as [`TransformOutput`].
    pub ast: &'ast Program<'ast>,
}

/// Result of the `transform` hook: the AST that replaces the carried one.
///
/// Returning `None` leaves the carried AST untouched; returning `Some`
/// replaces it for the rest of the chain. The returned AST must be rooted in
/// `args.allocator`.
#[derive(Debug)]
pub struct TransformOutput<'ast> {
    /// The replacement AST.
    pub ast: &'ast Program<'ast>,
}

/// The `transform` hook return: a replacement AST rooted in the compile
/// allocator, `None`, or an error. Generic over `'ast` because the returned
/// AST borrows the compile allocator.
pub type TransformReturn<'ast> = anyhow::Result<Option<TransformOutput<'ast>>>;

#[cfg(test)]
mod tests {
    use oxc::allocator::Allocator;
    use oxc::ast::ast::Program;

    use telarel_common::{CompileContext, ParseOptions, ParseResult, parse};

    use super::*;

    #[test]
    fn test_transform_output_wraps_program() {
        let allocator: Allocator = Allocator::default();

        let ctx: CompileContext<'_> = CompileContext::new("", "a.ts", "let a;");

        let parsed: ParseResult<'_> = parse(ParseOptions {
            context: &ctx,
            allocator: &allocator,
            file: "a.ts",
            code: "let a;",
            language: None,
            source_type: None,
        })
        .unwrap();

        let program: Program<'_> = parsed.program;

        let output: TransformOutput<'_> = TransformOutput { ast: &program };

        assert_eq!(output.ast.span.start, 0);
    }
}
