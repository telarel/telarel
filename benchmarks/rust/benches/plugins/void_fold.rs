use std::borrow::Cow;

use telarel::allocator::Allocator;
use telarel::ast::ast::{Expression, Program, UnaryOperator};
use telarel::ast::builder::AstBuilder;
use telarel::ast_visit::VisitMut;
use telarel::ast_visit::walk_mut;
use telarel::span::GetSpan;
use telarel::{
    Ast, HookUsage, Plugin, PluginContext, TransformArgs, TransformOutput,
    TransformReturn,
};

const NAME: &str = "undefined";

struct VoidFolder<'x> {
    builder: AstBuilder<'x>,
}

impl<'x> VoidFolder<'x> {
    fn is_void_zero(expression: &Expression<'x>) -> bool {
        let Expression::UnaryExpression(unary) = expression else {
            return false;
        };

        if unary.operator != UnaryOperator::Void {
            return false;
        }

        let Expression::NumericLiteral(literal) = &unary.argument else {
            return false;
        };

        literal.value == 0.0
    }
}

impl<'x> VisitMut<'x> for VoidFolder<'x> {
    fn visit_expression(
        &mut self,
        expression: &mut Expression<'x>,
    ) {
        if Self::is_void_zero(expression) {
            let span: telarel::span::Span = expression.span();
            *expression = Expression::new_identifier(span, NAME, &self.builder);
            return;
        }

        walk_mut::walk_expression(self, expression);
    }
}

#[derive(Debug)]
pub struct VoidFoldPlugin;

impl Plugin for VoidFoldPlugin {
    fn name(&self) -> Cow<'static, str> {
        "void-fold".into()
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
            let builder: AstBuilder<'_> = AstBuilder::new(allocator);

            let mut folder: VoidFolder<'_> = VoidFolder { builder };

            walk_mut::walk_program(&mut folder, program);
        });

        Ok(Some(TransformOutput { ast }))
    }
}
