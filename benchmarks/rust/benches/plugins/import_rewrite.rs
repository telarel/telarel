use std::borrow::Cow;

use telarel::allocator::Allocator;
use telarel::ast::ast::{ImportDeclaration, Program};
use telarel::ast_visit::VisitMut;
use telarel::ast_visit::walk_mut;
use telarel::str::Str;
use telarel::{
    Ast, HookUsage, Plugin, PluginContext, TransformArgs, TransformOutput,
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

    async fn transform<'a>(
        &'a self,
        _ctx: &'a PluginContext<'_>,
        args: TransformArgs<'a>,
    ) -> TransformReturn {
        let mut ast: Ast = args.ast.clone();

        ast.with_mut(|allocator: &Allocator, program: &mut Program<'_>| {
            let mut rewriter: ImportRewriter<'_> = ImportRewriter { allocator };
            walk_mut::walk_program(&mut rewriter, program);
        });

        Ok(Some(TransformOutput { ast }))
    }
}
