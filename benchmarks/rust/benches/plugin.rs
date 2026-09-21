use std::borrow::Cow;

use telarel::allocator::{ArenaBox, ArenaVec, CloneIn, GetAllocator};
use telarel::ast::ast::{
    Argument, ArrayExpression, ArrayExpressionElement, ArrowFunctionBody,
    ArrowFunctionExpression, BinaryExpression, BlockStatement, CallExpression,
    ComputedMemberExpression, ConditionalExpression, Directive, Expression,
    ExpressionStatement, FunctionBody, IdentifierName, IdentifierReference,
    IfStatement, JSXAttribute, JSXAttributeItem, JSXAttributeValue, JSXChild,
    JSXClosingElement, JSXElement, JSXElementName, JSXExpression,
    JSXExpressionContainer, JSXFragment, JSXIdentifier, JSXOpeningElement,
    JSXSpreadAttribute, JSXSpreadChild, LogicalExpression, ObjectExpression,
    ObjectProperty, ParenthesizedExpression, PrivateFieldExpression,
    PrivateIdentifier, Program, PropertyKey, ReturnStatement, SpreadElement,
    Statement, StaticMemberExpression, TemplateLiteral, VariableDeclaration,
    VariableDeclarator,
};
use telarel::ast::builder::AstBuilder;
use telarel::str::Ident;

const REPLACEMENT: &str = "Buttonx";

pub const TARGET: &str = "Button";

fn rename_statement<'a>(
    statement: &Statement<'a>,
    builder: &AstBuilder<'a>,
) -> Statement<'a> {
    match statement {
        | Statement::ExpressionStatement(expression_statement) => {
            let expression: Expression<'a> =
                rename_expression(&expression_statement.expression, builder);

            let node: ExpressionStatement<'a> = ExpressionStatement::new(
                expression_statement.span,
                expression,
                builder,
            );

            Statement::ExpressionStatement(ArenaBox::new_in(node, builder))
        },
        | Statement::ReturnStatement(return_statement) => {
            let argument: Option<Expression<'a>> = match &return_statement
                .argument
            {
                | Some(argument) => Some(rename_expression(argument, builder)),
                | None => None,
            };

            let node: ReturnStatement<'a> =
                ReturnStatement::new(return_statement.span, argument, builder);

            Statement::ReturnStatement(ArenaBox::new_in(node, builder))
        },
        | Statement::VariableDeclaration(declaration) => {
            let declarations: ArenaVec<'a, VariableDeclarator<'a>> =
                ArenaVec::from_iter_in(
                    declaration.declarations.iter().map(
                        |declarator: &VariableDeclarator<'a>| {
                            rename_declarator(declarator, builder)
                        },
                    ),
                    builder,
                );

            let node: VariableDeclaration<'a> = VariableDeclaration::new(
                declaration.span,
                declaration.kind,
                declarations,
                declaration.declare,
                builder,
            );

            Statement::VariableDeclaration(ArenaBox::new_in(node, builder))
        },
        | Statement::BlockStatement(block) => {
            let body: ArenaVec<'a, Statement<'a>> = ArenaVec::from_iter_in(
                block.body.iter().map(|statement: &Statement<'a>| {
                    rename_statement(statement, builder)
                }),
                builder,
            );

            let node: BlockStatement<'a> =
                BlockStatement::new(block.span, body, builder);

            Statement::BlockStatement(ArenaBox::new_in(node, builder))
        },
        | Statement::IfStatement(if_statement) => {
            let test: Expression<'a> =
                rename_expression(&if_statement.test, builder);

            let consequent: Statement<'a> =
                rename_statement(&if_statement.consequent, builder);

            let alternate: Option<Statement<'a>> = match &if_statement.alternate
            {
                | Some(alternate) => Some(rename_statement(alternate, builder)),
                | None => None,
            };

            let node: IfStatement<'a> = IfStatement::new(
                if_statement.span,
                test,
                consequent,
                alternate,
                builder,
            );

            Statement::IfStatement(ArenaBox::new_in(node, builder))
        },
        | other => other.clone_in(builder.allocator()),
    }
}

fn rename_expression<'a>(
    expression: &Expression<'a>,
    builder: &AstBuilder<'a>,
) -> Expression<'a> {
    match expression {
        | Expression::Identifier(identifier) => {
            let name: &str = identifier.name.as_str();

            let renamed: &str = if name == TARGET { REPLACEMENT } else { name };

            let ident: Ident<'a> = Ident::from_str_in(renamed, builder);

            let node: IdentifierReference<'a> =
                IdentifierReference::new(identifier.span, ident, builder);

            Expression::Identifier(ArenaBox::new_in(node, builder))
        },
        | Expression::CallExpression(call) => {
            let callee: Expression<'a> =
                rename_expression(&call.callee, builder);

            let arguments: ArenaVec<'a, Argument<'a>> = ArenaVec::from_iter_in(
                call.arguments.iter().map(|argument: &Argument<'a>| {
                    rename_argument(argument, builder)
                }),
                builder,
            );

            let node: CallExpression<'a> = CallExpression::new(
                call.span,
                callee,
                call.type_arguments.clone_in(builder.allocator()),
                arguments,
                call.optional,
                builder,
            );

            Expression::CallExpression(ArenaBox::new_in(node, builder))
        },
        | Expression::ComputedMemberExpression(member) => {
            let node: ComputedMemberExpression<'a> =
                rename_computed_member(member, builder);

            Expression::ComputedMemberExpression(ArenaBox::new_in(
                node, builder,
            ))
        },
        | Expression::StaticMemberExpression(member) => {
            let node: StaticMemberExpression<'a> =
                rename_static_member(member, builder);

            Expression::StaticMemberExpression(ArenaBox::new_in(node, builder))
        },
        | Expression::PrivateFieldExpression(member) => {
            let node: PrivateFieldExpression<'a> =
                rename_private_field(member, builder);

            Expression::PrivateFieldExpression(ArenaBox::new_in(node, builder))
        },
        | Expression::ConditionalExpression(conditional) => {
            let test: Expression<'a> =
                rename_expression(&conditional.test, builder);

            let consequent: Expression<'a> =
                rename_expression(&conditional.consequent, builder);

            let alternate: Expression<'a> =
                rename_expression(&conditional.alternate, builder);

            let node: ConditionalExpression<'a> = ConditionalExpression::new(
                conditional.span,
                test,
                consequent,
                alternate,
                builder,
            );

            Expression::ConditionalExpression(ArenaBox::new_in(node, builder))
        },
        | Expression::LogicalExpression(logical) => {
            let left: Expression<'a> =
                rename_expression(&logical.left, builder);

            let right: Expression<'a> =
                rename_expression(&logical.right, builder);

            let node: LogicalExpression<'a> = LogicalExpression::new(
                logical.span,
                left,
                logical.operator,
                right,
                builder,
            );

            Expression::LogicalExpression(ArenaBox::new_in(node, builder))
        },
        | Expression::BinaryExpression(binary) => {
            let left: Expression<'a> = rename_expression(&binary.left, builder);

            let right: Expression<'a> =
                rename_expression(&binary.right, builder);

            let node: BinaryExpression<'a> = BinaryExpression::new(
                binary.span,
                left,
                binary.operator,
                right,
                builder,
            );

            Expression::BinaryExpression(ArenaBox::new_in(node, builder))
        },
        | Expression::ArrowFunctionExpression(arrow) => {
            let body: ArrowFunctionBody<'a> = match &arrow.body {
                | ArrowFunctionBody::FunctionBody(function_body) => {
                    let statements: ArenaVec<'a, Statement<'a>> =
                        ArenaVec::from_iter_in(
                            function_body.statements.iter().map(
                                |statement: &Statement<'a>| {
                                    rename_statement(statement, builder)
                                },
                            ),
                            builder,
                        );

                    let directives: ArenaVec<'a, Directive<'a>> =
                        ArenaVec::from_iter_in(
                            function_body.directives.iter().map(
                                |directive: &Directive<'a>| {
                                    directive.clone_in(builder.allocator())
                                },
                            ),
                            builder,
                        );

                    let node: FunctionBody<'a> = FunctionBody::new(
                        function_body.span,
                        directives,
                        statements,
                        builder,
                    );

                    ArrowFunctionBody::FunctionBody(ArenaBox::new_in(
                        node, builder,
                    ))
                },
                | body => {
                    let expression: Expression<'a> = rename_expression(
                        body.as_expression()
                            .expect("arrow body must be an expression"),
                        builder,
                    );
                    ArrowFunctionBody::from(expression)
                },
            };

            let node: ArrowFunctionExpression<'a> =
                ArrowFunctionExpression::new(
                    arrow.span,
                    arrow.r#async,
                    arrow.type_parameters.clone_in(builder.allocator()),
                    arrow.params.clone_in(builder.allocator()),
                    arrow.return_type.clone_in(builder.allocator()),
                    body,
                    builder,
                );

            Expression::ArrowFunctionExpression(ArenaBox::new_in(node, builder))
        },
        | Expression::ObjectExpression(object) => {
            let properties: ArenaVec<
                'a,
                oxc::ast::ast::ObjectPropertyKind<'a>,
            > = ArenaVec::from_iter_in(
                object.properties.iter().map(
                    |property: &oxc::ast::ast::ObjectPropertyKind<'a>| {
                        rename_property_kind(property, builder)
                    },
                ),
                builder,
            );

            let node: ObjectExpression<'a> =
                ObjectExpression::new(object.span, properties, builder);

            Expression::ObjectExpression(ArenaBox::new_in(node, builder))
        },
        | Expression::ArrayExpression(array) => {
            let elements: ArenaVec<'a, ArrayExpressionElement<'a>> =
                ArenaVec::from_iter_in(
                    array.elements.iter().map(
                        |element: &ArrayExpressionElement<'a>| {
                            rename_array_element(element, builder)
                        },
                    ),
                    builder,
                );

            let node: ArrayExpression<'a> =
                ArrayExpression::new(array.span, elements, builder);

            Expression::ArrayExpression(ArenaBox::new_in(node, builder))
        },
        | Expression::TemplateLiteral(template) => {
            let expressions: ArenaVec<'a, Expression<'a>> =
                ArenaVec::from_iter_in(
                    template.expressions.iter().map(
                        |expression: &Expression<'a>| {
                            rename_expression(expression, builder)
                        },
                    ),
                    builder,
                );

            let node: TemplateLiteral<'a> = TemplateLiteral::new(
                template.span,
                template.quasis.clone_in(builder.allocator()),
                expressions,
                builder,
            );

            Expression::TemplateLiteral(ArenaBox::new_in(node, builder))
        },
        | Expression::ParenthesizedExpression(parenthesized) => {
            let inner: Expression<'a> =
                rename_expression(&parenthesized.expression, builder);

            let node: ParenthesizedExpression<'a> =
                ParenthesizedExpression::new(
                    parenthesized.span,
                    inner,
                    builder,
                );

            Expression::ParenthesizedExpression(ArenaBox::new_in(node, builder))
        },
        | Expression::JSXElement(element) => {
            Expression::JSXElement(rename_jsx_element(element, builder))
        },
        | Expression::JSXFragment(fragment) => {
            let children: ArenaVec<'a, JSXChild<'a>> = ArenaVec::from_iter_in(
                fragment.children.iter().map(|child: &JSXChild<'a>| {
                    rename_jsx_child(child, builder)
                }),
                builder,
            );

            let node: JSXFragment<'a> = JSXFragment::new(
                fragment.span,
                fragment.opening_fragment.clone_in(builder.allocator()),
                children,
                fragment.closing_fragment.clone_in(builder.allocator()),
                builder,
            );

            Expression::JSXFragment(ArenaBox::new_in(node, builder))
        },
        | other => other.clone_in(builder.allocator()),
    }
}

fn rename_declarator<'a>(
    declarator: &VariableDeclarator<'a>,
    builder: &AstBuilder<'a>,
) -> VariableDeclarator<'a> {
    let init: Option<Expression<'a>> = match &declarator.init {
        | Some(init) => Some(rename_expression(init, builder)),
        | None => None,
    };

    let node: VariableDeclarator<'a> = VariableDeclarator::new(
        declarator.span,
        declarator.id.clone_in(builder.allocator()),
        declarator.type_annotation.clone_in(builder.allocator()),
        init,
        declarator.definite,
        builder,
    );

    node
}

fn rename_computed_member<'a>(
    member: &ComputedMemberExpression<'a>,
    builder: &AstBuilder<'a>,
) -> ComputedMemberExpression<'a> {
    let object: Expression<'a> = rename_expression(&member.object, builder);

    let expression: Expression<'a> =
        rename_expression(&member.expression, builder);

    let node: ComputedMemberExpression<'a> = ComputedMemberExpression::new(
        member.span,
        object,
        expression,
        member.optional,
        builder,
    );

    node
}

fn rename_static_member<'a>(
    member: &StaticMemberExpression<'a>,
    builder: &AstBuilder<'a>,
) -> StaticMemberExpression<'a> {
    let object: Expression<'a> = rename_expression(&member.object, builder);

    let node: StaticMemberExpression<'a> = StaticMemberExpression::new(
        member.span,
        object,
        member.property.clone_in(builder.allocator()),
        member.optional,
        builder,
    );

    node
}

fn rename_private_field<'a>(
    member: &PrivateFieldExpression<'a>,
    builder: &AstBuilder<'a>,
) -> PrivateFieldExpression<'a> {
    let object: Expression<'a> = rename_expression(&member.object, builder);

    let node: PrivateFieldExpression<'a> = PrivateFieldExpression::new(
        member.span,
        object,
        member.field.clone_in(builder.allocator()),
        member.optional,
        builder,
    );

    node
}

fn rename_property_kind<'a>(
    property: &oxc::ast::ast::ObjectPropertyKind<'a>,
    builder: &AstBuilder<'a>,
) -> oxc::ast::ast::ObjectPropertyKind<'a> {
    match property {
        | oxc::ast::ast::ObjectPropertyKind::ObjectProperty(
            object_property,
        ) => {
            let key: PropertyKey<'a> = match object_property.computed {
                | true => rename_property_key(&object_property.key, builder),
                | false => object_property.key.clone_in(builder.allocator()),
            };

            let value: Expression<'a> =
                rename_expression(&object_property.value, builder);

            let node: ObjectProperty<'a> = ObjectProperty::new(
                object_property.span,
                object_property.kind,
                key,
                value,
                object_property.method,
                object_property.shorthand,
                object_property.computed,
                builder,
            );

            oxc::ast::ast::ObjectPropertyKind::ObjectProperty(ArenaBox::new_in(
                node, builder,
            ))
        },
        | oxc::ast::ast::ObjectPropertyKind::SpreadProperty(spread) => {
            let argument: Expression<'a> =
                rename_expression(&spread.argument, builder);
            let node: SpreadElement<'a> =
                SpreadElement::new(spread.span, argument, builder);
            oxc::ast::ast::ObjectPropertyKind::SpreadProperty(ArenaBox::new_in(
                node, builder,
            ))
        },
    }
}

fn rename_property_key<'a>(
    key: &PropertyKey<'a>,
    builder: &AstBuilder<'a>,
) -> PropertyKey<'a> {
    match key {
        | PropertyKey::StaticIdentifier(identifier) => {
            let node: IdentifierName<'a> = IdentifierName::new(
                identifier.span,
                identifier.name.clone_in(builder.allocator()),
                builder,
            );
            PropertyKey::StaticIdentifier(ArenaBox::new_in(node, builder))
        },
        | PropertyKey::PrivateIdentifier(identifier) => {
            let node: PrivateIdentifier<'a> = PrivateIdentifier::new(
                identifier.span,
                identifier.name.clone_in(builder.allocator()),
                builder,
            );
            PropertyKey::PrivateIdentifier(ArenaBox::new_in(node, builder))
        },
        | key => {
            let expression: &Expression<'a> = key
                .as_expression()
                .expect("property key must be an expression");
            PropertyKey::from(rename_expression(expression, builder))
        },
    }
}

fn rename_array_element<'a>(
    element: &ArrayExpressionElement<'a>,
    builder: &AstBuilder<'a>,
) -> ArrayExpressionElement<'a> {
    match element {
        | ArrayExpressionElement::SpreadElement(spread) => {
            let argument: Expression<'a> =
                rename_expression(&spread.argument, builder);
            let node: SpreadElement<'a> =
                SpreadElement::new(spread.span, argument, builder);
            ArrayExpressionElement::SpreadElement(ArenaBox::new_in(
                node, builder,
            ))
        },
        | ArrayExpressionElement::Elision(_) => {
            element.clone_in(builder.allocator())
        },
        | element => {
            let expression: &Expression<'a> = element
                .as_expression()
                .expect("array element must be an expression");
            ArrayExpressionElement::from(rename_expression(expression, builder))
        },
    }
}

fn rename_argument<'a>(
    argument: &Argument<'a>,
    builder: &AstBuilder<'a>,
) -> Argument<'a> {
    match argument {
        | Argument::SpreadElement(spread) => {
            let inner: Expression<'a> =
                rename_expression(&spread.argument, builder);

            let node: SpreadElement<'a> =
                SpreadElement::new(spread.span, inner, builder);

            Argument::SpreadElement(ArenaBox::new_in(node, builder))
        },
        | argument => {
            let expression: &Expression<'a> = argument
                .as_expression()
                .expect("argument must be an expression");
            Argument::from(rename_expression(expression, builder))
        },
    }
}

fn rename_jsx_element_name<'a>(
    name: &JSXElementName<'a>,
    builder: &AstBuilder<'a>,
) -> JSXElementName<'a> {
    match name {
        | JSXElementName::Identifier(identifier) => {
            let node: JSXIdentifier<'a> = JSXIdentifier::new(
                identifier.span,
                identifier.name.clone_in(builder.allocator()),
                builder,
            );

            JSXElementName::Identifier(ArenaBox::new_in(node, builder))
        },
        | JSXElementName::IdentifierReference(identifier) => {
            let node_name: &str = identifier.name.as_str();

            let renamed: &str =
                if node_name == TARGET { REPLACEMENT } else { node_name };

            let ident: Ident<'a> = Ident::from_str_in(renamed, builder);

            JSXElementName::new_identifier_reference(
                identifier.span,
                ident,
                builder,
            )
        },
        | other => other.clone_in(builder.allocator()),
    }
}

fn rename_jsx_expression_container<'a>(
    container: &JSXExpressionContainer<'a>,
    builder: &AstBuilder<'a>,
) -> JSXExpressionContainer<'a> {
    let expression: JSXExpression<'a> = match &container.expression {
        | JSXExpression::EmptyExpression(_) => {
            container.expression.clone_in(builder.allocator())
        },
        | inner => JSXExpression::from(rename_expression(
            inner
                .as_expression()
                .expect("jsx expression must be an expression"),
            builder,
        )),
    };

    let node: JSXExpressionContainer<'a> =
        JSXExpressionContainer::new(container.span, expression, builder);

    node
}

fn rename_jsx_attribute_item<'a>(
    item: &JSXAttributeItem<'a>,
    builder: &AstBuilder<'a>,
) -> JSXAttributeItem<'a> {
    match item {
        | JSXAttributeItem::SpreadAttribute(spread) => {
            let argument: Expression<'a> =
                rename_expression(&spread.argument, builder);

            let node: JSXSpreadAttribute<'a> =
                JSXSpreadAttribute::new(spread.span, argument, builder);

            JSXAttributeItem::SpreadAttribute(ArenaBox::new_in(node, builder))
        },
        | JSXAttributeItem::Attribute(attribute) => {
            let value: Option<JSXAttributeValue<'a>> = match &attribute.value {
                | Some(JSXAttributeValue::ExpressionContainer(container)) => {
                    let node: JSXExpressionContainer<'a> =
                        rename_jsx_expression_container(container, builder);

                    Some(JSXAttributeValue::ExpressionContainer(
                        ArenaBox::new_in(node, builder),
                    ))
                },
                | Some(JSXAttributeValue::Element(element)) => {
                    let renamed: ArenaBox<'a, JSXElement<'a>> =
                        rename_jsx_element(element, builder);
                    Some(JSXAttributeValue::Element(renamed))
                },
                | Some(JSXAttributeValue::Fragment(fragment)) => {
                    let children: ArenaVec<'a, JSXChild<'a>> =
                        ArenaVec::from_iter_in(
                            fragment.children.iter().map(
                                |child: &JSXChild<'a>| {
                                    rename_jsx_child(child, builder)
                                },
                            ),
                            builder,
                        );

                    let node: JSXFragment<'a> = JSXFragment::new(
                        fragment.span,
                        fragment.opening_fragment.clone_in(builder.allocator()),
                        children,
                        fragment.closing_fragment.clone_in(builder.allocator()),
                        builder,
                    );

                    Some(JSXAttributeValue::Fragment(ArenaBox::new_in(
                        node, builder,
                    )))
                },
                | other => other.clone_in(builder.allocator()),
            };
            let node: JSXAttribute<'a> = JSXAttribute::new(
                attribute.span,
                attribute.name.clone_in(builder.allocator()),
                value,
                builder,
            );
            JSXAttributeItem::Attribute(ArenaBox::new_in(node, builder))
        },
    }
}

fn rename_jsx_child<'a>(
    child: &JSXChild<'a>,
    builder: &AstBuilder<'a>,
) -> JSXChild<'a> {
    match child {
        | JSXChild::Element(element) => {
            JSXChild::Element(rename_jsx_element(element, builder))
        },
        | JSXChild::Fragment(fragment) => {
            let children: ArenaVec<'a, JSXChild<'a>> = ArenaVec::from_iter_in(
                fragment.children.iter().map(|child: &JSXChild<'a>| {
                    rename_jsx_child(child, builder)
                }),
                builder,
            );
            let node: JSXFragment<'a> = JSXFragment::new(
                fragment.span,
                fragment.opening_fragment.clone_in(builder.allocator()),
                children,
                fragment.closing_fragment.clone_in(builder.allocator()),
                builder,
            );
            JSXChild::Fragment(ArenaBox::new_in(node, builder))
        },
        | JSXChild::ExpressionContainer(container) => {
            let node: JSXExpressionContainer<'a> =
                rename_jsx_expression_container(container, builder);
            JSXChild::ExpressionContainer(ArenaBox::new_in(node, builder))
        },
        | JSXChild::Spread(spread) => {
            let expression: Expression<'a> =
                rename_expression(&spread.expression, builder);
            let node: JSXSpreadChild<'a> =
                JSXSpreadChild::new(spread.span, expression, builder);
            JSXChild::Spread(ArenaBox::new_in(node, builder))
        },
        | other => other.clone_in(builder.allocator()),
    }
}

fn rename_jsx_element<'a>(
    element: &JSXElement<'a>,
    builder: &AstBuilder<'a>,
) -> ArenaBox<'a, JSXElement<'a>> {
    let opening: &JSXOpeningElement<'a> = &element.opening_element;

    let name: JSXElementName<'a> =
        rename_jsx_element_name(&opening.name, builder);

    let attributes: ArenaVec<'a, JSXAttributeItem<'a>> = ArenaVec::from_iter_in(
        opening.attributes.iter().map(|item: &JSXAttributeItem<'a>| {
            rename_jsx_attribute_item(item, builder)
        }),
        builder,
    );

    let opening_element: JSXOpeningElement<'a> = JSXOpeningElement::new(
        opening.span,
        name,
        opening.type_arguments.clone_in(builder.allocator()),
        attributes,
        builder,
    );

    let children: ArenaVec<'a, JSXChild<'a>> = ArenaVec::from_iter_in(
        element
            .children
            .iter()
            .map(|child: &JSXChild<'a>| rename_jsx_child(child, builder)),
        builder,
    );

    let closing_element: Option<JSXClosingElement<'a>> =
        match &element.closing_element {
            | Some(closing) => {
                let name: JSXElementName<'a> =
                    rename_jsx_element_name(&closing.name, builder);
                let node: JSXClosingElement<'a> =
                    JSXClosingElement::new(closing.span, name, builder);
                Some(node)
            },
            | None => None,
        };

    let node: JSXElement<'a> = JSXElement::new(
        element.span,
        ArenaBox::new_in(opening_element, builder),
        children,
        closing_element.map(|closing| ArenaBox::new_in(closing, builder)),
        builder,
    );

    ArenaBox::new_in(node, builder)
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

    async fn transform<'a>(
        &'a self,
        _: &'a telarel::CompileContext<'a>,
        args: &'a telarel::TransformArgs<'a>,
    ) -> telarel::TransformReturn<'a> {
        let builder: AstBuilder<'a> = AstBuilder::new(args.allocator);

        let original: &'a Program<'a> = args.program;

        let body: ArenaVec<'a, Statement<'a>> = ArenaVec::from_iter_in(
            original.body.iter().map(|statement: &Statement<'a>| {
                rename_statement(statement, &builder)
            }),
            &builder,
        );

        let directives: ArenaVec<'a, Directive<'a>> = ArenaVec::from_iter_in(
            original.directives.iter().map(|directive: &Directive<'a>| {
                directive.clone_in(builder.allocator())
            }),
            &builder,
        );

        let program: Program<'a> = Program::new(
            original.span,
            original.source_type,
            original.source_text,
            original.comments.clone_in(builder.allocator()),
            original.hashbang.clone_in(builder.allocator()),
            directives,
            body,
            &builder,
        );

        Ok(Some(telarel::TransformOutput { program }))
    }
}
