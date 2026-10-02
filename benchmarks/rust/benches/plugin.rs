use std::borrow::Cow;

use telarel::allocator::Allocator;
use telarel::allocator::CloneIn;
use telarel::ast::ast::{IdentifierReference, JSXIdentifier};
use telarel::ast_visit::VisitMut;
use telarel::ast_visit::walk_mut;
use telarel::str::{Ident, Str};

pub const TARGET: &str = "Button";

const REPLACEMENT: &str = "Buttonx";

struct Renamer<'x> {
    allocator: &'x Allocator,
}

impl<'x> VisitMut<'x> for Renamer<'x> {
    fn visit_identifier_reference(
        &mut self,
        ident: &mut IdentifierReference<'x>,
    ) {
        if ident.name.as_str() == TARGET {
            ident.name = Ident::from_str_in(REPLACEMENT, &self.allocator);
        }
    }

    fn visit_jsx_identifier(
        &mut self,
        ident: &mut JSXIdentifier<'x>,
    ) {
        if ident.name.as_str() == TARGET {
            ident.name = Str::from_str_in(REPLACEMENT, &self.allocator);
        }
    }
}

#[derive(Debug)]
pub struct TransformPlugin;

impl telarel::Plugin for TransformPlugin {
    fn name(&self) -> Cow<'static, str> {
        "rename-jsx-elements".into()
    }

    fn register_hook_usage(&self) -> telarel::HookUsage {
        telarel::HookUsage::Transform
    }

    async fn transform<'a, 'ast: 'a>(
        &'a self,
        _ctx: &'a telarel::PluginContext<'a>,
        args: telarel::TransformArgs<'ast>,
    ) -> telarel::TransformReturn<'ast> {
        let mut current: telarel::ast::ast::Program<'ast> =
            (*args.ast).clone_in(args.allocator);

        let mut renamer: Renamer<'ast> = Renamer { allocator: args.allocator };

        walk_mut::walk_program(&mut renamer, &mut current);

        let ast: &'ast telarel::ast::ast::Program<'ast> =
            args.allocator.alloc(current);

        Ok(Some(telarel::TransformOutput { ast }))
    }
}
