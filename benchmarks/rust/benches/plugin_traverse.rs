use std::borrow::Cow;

use telarel::allocator::Allocator;
use telarel::ast::ast::{IdentifierReference, JSXIdentifier};
use telarel::str::{Ident, Str};
use telarel::traverse::{Traverse, TraverseCtx, traverse_mut};

pub const TARGET: &str = "Button";

const REPLACEMENT: &str = "Buttonx";

struct TraverseRenamer<'x> {
    allocator: &'x Allocator,
}

impl<'x> Traverse<'x, ()> for TraverseRenamer<'x> {
    fn enter_identifier_reference(
        &mut self,
        ident: &mut IdentifierReference<'x>,
        _ctx: &mut TraverseCtx<'x, ()>,
    ) {
        if ident.name.as_str() == TARGET {
            ident.name = Ident::from_str_in(REPLACEMENT, &self.allocator);
        }
    }

    fn enter_jsx_identifier(
        &mut self,
        ident: &mut JSXIdentifier<'x>,
        _ctx: &mut TraverseCtx<'x, ()>,
    ) {
        if ident.name.as_str() == TARGET {
            ident.name = Str::from_str_in(REPLACEMENT, &self.allocator);
        }
    }
}

#[derive(Debug)]
pub struct TraversePlugin;

impl telarel::Plugin for TraversePlugin {
    fn name(&self) -> Cow<'static, str> {
        "rename-jsx-elements".into()
    }

    fn register_hook_usage(&self) -> telarel::HookUsage {
        telarel::HookUsage::Transform
    }

    async fn transform<'a, 'ast>(
        &'a self,
        _: &'a telarel::CompileContext<'a>,
        args: telarel::TransformArgs<'a, 'ast>,
    ) -> telarel::TransformReturn {
        let mut renamer: TraverseRenamer<'_> =
            TraverseRenamer { allocator: args.allocator };

        // `oxc_traverse` reads scope IDs stamped onto AST nodes during the
        // semantic pass, so build a `Scoping` for the program instead of
        // using a default (empty) one (mirrors oxc's `rebuild_scoping`).
        let scoping: telarel::semantic::Scoping =
            telarel::semantic::SemanticBuilder::new()
                .build(args.program)
                .semantic
                .into_scoping();

        traverse_mut(&mut renamer, args.allocator, args.program, scoping, ());

        Ok(())
    }
}
