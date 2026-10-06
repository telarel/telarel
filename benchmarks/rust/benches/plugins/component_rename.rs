use std::borrow::Cow;

use telarel::allocator::Allocator;
use telarel::allocator::CloneIn;
use telarel::ast::ast::{
    BindingIdentifier, IdentifierReference, JSXIdentifier, Program,
};
use telarel::ast_visit::VisitMut;
use telarel::ast_visit::walk_mut;
use telarel::str::{Ident, Str};
use telarel::{
    HookUsage, Plugin, PluginContext, TransformArgs, TransformOutput,
    TransformReturn,
};

const FROM: &str = "Button";

const TO: &str = "Pressable";

struct Renamer<'x> {
    allocator: &'x Allocator,
}

impl<'x> VisitMut<'x> for Renamer<'x> {
    fn visit_identifier_reference(
        &mut self,
        ident: &mut IdentifierReference<'x>,
    ) {
        if ident.name.as_str() == FROM {
            ident.name = Ident::from_str_in(TO, &self.allocator);
        }
    }

    fn visit_binding_identifier(
        &mut self,
        ident: &mut BindingIdentifier<'x>,
    ) {
        if ident.name.as_str() == FROM {
            ident.name = Ident::from_str_in(TO, &self.allocator);
        }
    }

    fn visit_jsx_identifier(
        &mut self,
        ident: &mut JSXIdentifier<'x>,
    ) {
        if ident.name.as_str() == FROM {
            ident.name = Str::from_str_in(TO, &self.allocator);
        }
    }
}

#[derive(Debug)]
pub struct ComponentRenamePlugin;

impl Plugin for ComponentRenamePlugin {
    fn name(&self) -> Cow<'static, str> {
        "component-rename".into()
    }

    fn register_hook_usage(&self) -> HookUsage {
        HookUsage::Transform
    }

    async fn transform<'a, 'ast: 'a>(
        &'a self,
        _ctx: &'a PluginContext<'a>,
        args: TransformArgs<'ast>,
    ) -> TransformReturn<'ast> {
        let mut current: Program<'ast> = (*args.ast).clone_in(args.allocator);

        let mut renamer: Renamer<'ast> = Renamer { allocator: args.allocator };

        walk_mut::walk_program(&mut renamer, &mut current);

        let ast: &'ast Program<'ast> = args.allocator.alloc(current);

        Ok(Some(TransformOutput { ast }))
    }
}
