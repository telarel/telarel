use std::borrow::Cow;

use telarel::allocator::Allocator;
use telarel::ast::ast::{
    BindingIdentifier, IdentifierReference, JSXIdentifier, Program,
};
use telarel::ast_visit::VisitMut;
use telarel::ast_visit::walk_mut;
use telarel::str::{Ident, Str};
use telarel::{
    Ast, HookUsage, Plugin, PluginContext, TransformArgs, TransformOutput,
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

    async fn transform<'a>(
        &'a self,
        _ctx: &'a PluginContext<'_>,
        args: TransformArgs<'a>,
    ) -> TransformReturn {
        let mut ast: Ast = args.ast.clone();

        ast.with_mut(|allocator: &Allocator, program: &mut Program<'_>| {
            let mut renamer: Renamer<'_> = Renamer { allocator };
            walk_mut::walk_program(&mut renamer, program);
        });

        Ok(Some(TransformOutput { ast }))
    }
}
