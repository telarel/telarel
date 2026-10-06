use std::borrow::Cow;

use telarel::allocator::CloneIn;
use telarel::ast::ast::{Expression, Program, UnaryOperator};
use telarel::ast::builder::AstBuilder;
use telarel::ast_visit::VisitMut;
use telarel::ast_visit::walk_mut;
use telarel::span::GetSpan;
use telarel::{
    HookUsage, Plugin, PluginContext, TransformArgs, TransformOutput,
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

    async fn transform<'a, 'ast: 'a>(
        &'a self,
        _ctx: &'a PluginContext<'a>,
        args: TransformArgs<'ast>,
    ) -> TransformReturn<'ast> {
        let mut current: Program<'ast> = (*args.ast).clone_in(args.allocator);

        let builder: AstBuilder<'ast> = AstBuilder::new(args.allocator);

        let mut folder: VoidFolder<'ast> = VoidFolder { builder };

        walk_mut::walk_program(&mut folder, &mut current);

        let ast: &'ast Program<'ast> = args.allocator.alloc(current);

        Ok(Some(TransformOutput { ast }))
    }
}
