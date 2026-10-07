use std::borrow::Cow;

use telarel::allocator::Allocator;
use telarel::ast::ast::{
    JSXAttributeItem, JSXAttributeName, JSXAttributeValue, JSXOpeningElement,
    Program,
};
use telarel::ast::builder::AstBuilder;
use telarel::ast_visit::VisitMut;
use telarel::ast_visit::walk_mut;
use telarel::span::SPAN;
use telarel::{
    Ast, HookUsage, Plugin, PluginContext, TransformArgs, TransformOutput,
    TransformReturn,
};

const NAME: &str = "data-telarel";

const VALUE: &str = "true";

struct AttributeInjector<'x> {
    builder: AstBuilder<'x>,
}

impl<'x> VisitMut<'x> for AttributeInjector<'x> {
    fn visit_jsx_opening_element(
        &mut self,
        element: &mut JSXOpeningElement<'x>,
    ) {
        let name: JSXAttributeName<'x> =
            JSXAttributeName::new_identifier(SPAN, NAME, &self.builder);

        let value: JSXAttributeValue<'x> =
            JSXAttributeValue::new_string_literal(
                SPAN,
                VALUE,
                None,
                &self.builder,
            );

        let attribute: JSXAttributeItem<'x> = JSXAttributeItem::new_attribute(
            SPAN,
            name,
            Some(value),
            &self.builder,
        );

        element.attributes.push(attribute);

        walk_mut::walk_jsx_opening_element(self, element);
    }
}

#[derive(Debug)]
pub struct JsxAttributePlugin;

impl Plugin for JsxAttributePlugin {
    fn name(&self) -> Cow<'static, str> {
        "jsx-attribute".into()
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

            let mut injector: AttributeInjector<'_> =
                AttributeInjector { builder };

            walk_mut::walk_program(&mut injector, program);
        });

        Ok(Some(TransformOutput { ast }))
    }
}
