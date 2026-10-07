use std::sync::Arc;

use oxc::allocator::{Allocator, CloneIn};
use oxc::ast::ast::Program;
use oxc::span::SourceType;

struct AstOwner {
    source: Arc<str>,
    allocator: Allocator,
    source_type: SourceType,
}

struct AstDependent<'cell> {
    program: Program<'cell>,
}

self_cell::self_cell!(
    struct AstInner {
        owner: AstOwner,

        #[covariant]
        dependent: AstDependent,
    }
);

/// An owned, self-referential AST that bundles its own allocator.
///
/// `Ast` is `Sync` for driver plumbing: the transform hook's future is `Send`
/// and borrows `&Ast`, so the AST must cross threads by shared reference.
///
/// That `Sync` does not make concurrent mutation safe. `Program`
/// exposes interior-mutable `Cell` fields (`node_id`, `scope_id`) that safe code
/// can mutate through the shared reference returned by [`Ast::program`]. Sharing
/// `&Ast` across threads is sound only under the caller-side contract that it is
/// used read-only: no thread may mutate `Program`'s `Cell` fields through a
/// shared `&Ast`. Doing so is a data race and is forbidden.
pub struct Ast {
    inner: AstInner,
}

impl Ast {
    /// Build an owned AST whose `Program::source_text` borrows the owner's source.
    pub fn from_source<F>(
        source: Arc<str>,
        source_type: SourceType,
        build: F,
    ) -> Ast
    where
        F: for<'a> FnOnce(&'a str, &'a Allocator) -> Program<'a>,
    {
        let owner: AstOwner =
            AstOwner { source, allocator: Allocator::default(), source_type };

        let inner: AstInner = AstInner::new(owner, |owner: &AstOwner| {
            AstDependent { program: build(&owner.source, &owner.allocator) }
        });

        Ast { inner }
    }

    /// Fallible variant of [`Ast::from_source`] (parse / ESTree read-back).
    pub fn try_from_source<F, E>(
        source: Arc<str>,
        source_type: SourceType,
        build: F,
    ) -> Result<Ast, E>
    where
        F: for<'a> FnOnce(&'a str, &'a Allocator) -> Result<Program<'a>, E>,
    {
        let owner: AstOwner =
            AstOwner { source, allocator: Allocator::default(), source_type };

        let inner: AstInner = AstInner::try_new(owner, |owner: &AstOwner| {
            let program: Program<'_> = build(&owner.source, &owner.allocator)?;

            Ok(AstDependent { program })
        })?;

        Ok(Ast { inner })
    }

    /// Borrow the read-only program.
    pub fn program(&self) -> &Program<'_> {
        &self.inner.borrow_dependent().program
    }

    /// Borrow the owned source.
    pub fn source(&self) -> &Arc<str> {
        &self.inner.borrow_owner().source
    }

    /// The resolved source type.
    pub fn source_type(&self) -> SourceType {
        self.inner.borrow_owner().source_type
    }

    /// The only path to the arena: an exclusive borrow, so the closure
    /// cannot race with another thread.
    pub fn with_mut<R>(
        &mut self,
        f: impl for<'i> FnOnce(&'i Allocator, &mut Program<'i>) -> R,
    ) -> R {
        self.inner.with_dependent_mut(|owner, dep| {
            f(&owner.allocator, &mut dep.program)
        })
    }
}

impl std::fmt::Debug for Ast {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        f.debug_struct("Ast").field("source_len", &self.source().len()).finish()
    }
}

impl Clone for Ast {
    /// Clone into a fresh allocator, sharing the `Arc<str>` source.
    fn clone(&self) -> Ast {
        let owner: &AstOwner = self.inner.borrow_owner();

        let source: Arc<str> = Arc::clone(&owner.source);
        let source_type: SourceType = owner.source_type;
        let capacity: usize = owner.allocator.used_bytes();

        let owner: AstOwner = AstOwner {
            source,
            allocator: Allocator::with_capacity(capacity),
            source_type,
        };

        let inner: AstInner = AstInner::new(owner, |owner: &AstOwner| {
            AstDependent { program: self.program().clone_in(&owner.allocator) }
        });

        Ast { inner }
    }
}

// SAFETY: owner + dependent are heap-boxed together by `self_cell` and move as
// one; `Allocator` (bumpalo `Bump`) is `Send` and `Arc<str>` is `Send`, so no
// thread-affine pointer dangles when the `Ast` moves.
unsafe impl Send for Ast {}

// SAFETY: two separate guarantees are required, and they are not the same:
//
// (a) The arena/`Allocator` is safe because it is only reachable through
//     `with_mut(&mut self, ..)`, an exclusive borrow, and the driver runs
//     plugins sequentially. There is intentionally no public `&Allocator`
//     accessor, so this invariant is not bypassable through safe code.
//
// (b) This justification does NOT cover `Program`'s own interior mutability.
//     `Program` exposes `pub node_id: Cell<NodeId>` and
//     `pub scope_id: Cell<Option<ScopeId>>`, which safe code can mutate through
//     the shared reference returned by `program()`. Because `Ast: Sync`, `&Ast`
//     is `Send`, so such a mutation from two threads would be a data race. This
//     impl is therefore sound ONLY under the additional caller-side contract
//     that the AST is shared across threads read-only: no one mutates
//     `Program`'s pub `Cell` fields through a shared `&Ast`.
//
// The plan mandates `Sync` because the transform hook's `Send` future borrows
// `&Ast` (`TransformArgs<'a> { ast: &'a Ast }`).
unsafe impl Sync for Ast {}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use oxc::parser::Parser;
    use oxc::span::SourceType as OxcSourceType;

    use super::*;

    fn build_program<'a>(
        source: &'a str,
        allocator: &'a Allocator,
    ) -> Program<'a> {
        Parser::new(allocator, source, OxcSourceType::ts()).parse().program
    }

    fn owner_from_code(code: &str) -> Ast {
        let source: Arc<str> = Arc::from(code);

        Ast::from_source(source, OxcSourceType::ts(), build_program)
    }

    #[test]
    fn test_from_source_builds_program() {
        let ast: Ast = owner_from_code("const a = 1;");

        assert!(!ast.program().body.is_empty());
        assert_eq!(ast.source().as_ref(), "const a = 1;");
        assert!(ast.source_type().is_typescript());
    }

    #[test]
    fn test_try_from_source_propagates_error() {
        let result: Result<Ast, &'static str> = Ast::try_from_source(
            Arc::from("const a = 1;"),
            OxcSourceType::ts(),
            |_source: &str, _allocator: &Allocator| Err("boom"),
        );

        assert_eq!(result.err(), Some("boom"));
    }

    #[test]
    fn test_try_from_source_builds_program() {
        let result: Result<Ast, &'static str> = Ast::try_from_source(
            Arc::from("const a = 1;"),
            OxcSourceType::ts(),
            |source: &str, allocator: &Allocator| {
                Ok(build_program(source, allocator))
            },
        );

        let ast: Ast = result.expect("valid source parses");

        assert!(!ast.program().body.is_empty());
    }

    #[test]
    fn test_program_source_text_borrows_owner_source() {
        let ast: Ast = owner_from_code("const a = 1;");

        assert_eq!(ast.program().source_text, ast.source().as_ref());
    }

    #[test]
    fn test_with_mut_can_mutate_program() {
        let mut ast: Ast = owner_from_code("const a = 1;");

        ast.with_mut(|allocator: &Allocator, program: &mut Program<'_>| {
            program.source_text = allocator.alloc_str("// changed");
        });

        assert_eq!(ast.program().source_text, "// changed");
    }

    #[test]
    fn test_clone_round_trips_program() {
        let ast: Ast = owner_from_code("const a = 1;");

        let clone: Ast = ast.clone();

        assert_eq!(clone.program().source_text, ast.program().source_text);
        assert_eq!(clone.program().body.len(), ast.program().body.len());
        assert_eq!(clone.source_type(), ast.source_type());
    }

    #[test]
    fn test_clone_shares_source_arc() {
        let ast: Ast = owner_from_code("const a = 1;");

        let clone: Ast = ast.clone();

        assert!(Arc::ptr_eq(ast.source(), clone.source()));
    }

    #[test]
    fn test_debug_prints_source_length() {
        let ast: Ast = owner_from_code("const a = 1;");

        let debug: String = format!("{:?}", ast);

        assert!(debug.contains("Ast"));
        assert!(debug.contains("source_len"));
        assert!(debug.contains("12"));
    }

    #[test]
    fn test_ast_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}

        assert_send_sync::<Ast>();
    }
}
