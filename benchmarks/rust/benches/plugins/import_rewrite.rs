use std::borrow::Cow;

use telarel::allocator::Allocator;
use telarel::allocator::CloneIn;
use telarel::ast::ast::{ImportDeclaration, Program};
use telarel::ast_visit::VisitMut;
use telarel::ast_visit::walk_mut;
use telarel::str::Str;
use telarel::{
    HookUsage, Plugin, PluginContext, TransformArgs, TransformOutput,
    TransformReturn,
};

const FROM: &str = "react";

const TO: &str = "preact";

struct ImportRewriter<'x> {
    allocator: &'x Allocator,
}

impl<'x> VisitMut<'x> for ImportRewriter<'x> {
    fn visit_import_declaration(
        &mut self,
        declaration: &mut ImportDeclaration<'x>,
    ) {
        if declaration.source.value.as_str() == FROM {
            declaration.source.value = Str::from_str_in(TO, &self.allocator);
        }
    }
}

#[derive(Debug)]
pub struct ImportRewritePlugin;

impl Plugin for ImportRewritePlugin {
    fn name(&self) -> Cow<'static, str> {
        "import-rewrite".into()
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

        let mut rewriter: ImportRewriter<'ast> =
            ImportRewriter { allocator: args.allocator };

        walk_mut::walk_program(&mut rewriter, &mut current);

        let ast: &'ast Program<'ast> = args.allocator.alloc(current);

        Ok(Some(TransformOutput { ast }))
    }
}
