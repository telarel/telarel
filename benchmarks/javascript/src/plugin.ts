import type {
    Plugin,
    PluginContext,
    TransformArgs,
    TransformOutput,
} from "telarel";
import type {
    Argument,
    ArrayExpression,
    ArrayExpressionElement,
    ArrowFunctionExpression,
    BlockStatement,
    CallExpression,
    ConditionalExpression,
    Directive,
    Expression,
    ExpressionStatement,
    FunctionBody,
    IdentifierReference,
    IfStatement,
    JSXAttribute,
    JSXAttributeItem,
    JSXChild,
    JSXElement,
    JSXElementName,
    JSXExpressionContainer,
    JSXFragment,
    LogicalExpression,
    MemberExpression,
    ObjectExpression,
    ObjectProperty,
    ParenthesizedExpression,
    PrivateFieldExpression,
    Program,
    ReturnStatement,
    SpreadElement,
    Statement,
    StaticMemberExpression,
    TemplateLiteral,
    VariableDeclaration,
    VariableDeclarator,
} from "telarel/ast";

const TARGET: string = "Button";

const RENAMED: string = `${TARGET}x`;

const renameJsxName = (name: JSXElementName): JSXElementName => {
    if (name.type === "JSXIdentifier" && name.name === TARGET) {
        return { ...name, name: RENAMED };
    }
    return name;
};

const renameExpression = (expression: Expression): Expression => {
    if (expression.type === "JSXElement") {
        const element: JSXElement = expression;
        return {
            ...element,
            openingElement: {
                ...element.openingElement,
                name: renameJsxName(element.openingElement.name),
                attributes:
                    element.openingElement.attributes.map(renameAttributeItem),
            },
            closingElement:
                element.closingElement === null
                    ? null
                    : {
                          ...element.closingElement,
                          name: renameJsxName(element.closingElement.name),
                      },
            children: element.children.map(renameJsxChild),
        };
    }

    if (expression.type === "JSXFragment") {
        const fragment: JSXFragment = expression;
        return {
            ...fragment,
            children: fragment.children.map(renameJsxChild),
        };
    }

    if (expression.type === "Identifier") {
        const identifier: IdentifierReference = expression;

        const name: string =
            identifier.name === TARGET ? RENAMED : identifier.name;

        return { ...identifier, name };
    }

    if (expression.type === "CallExpression") {
        const call: CallExpression = expression;

        return {
            ...call,
            callee: renameExpression(call.callee),
            arguments: call.arguments.map(renameArgument),
        };
    }

    if (expression.type === "MemberExpression") {
        const member: MemberExpression = expression;
        if (member.computed) {
            return {
                ...member,
                object: renameExpression(member.object),
                property: renameExpression(member.property),
            };
        }
        if (member.property.type !== "Identifier") return expression;
        const staticMember: StaticMemberExpression | PrivateFieldExpression =
            member;
        return {
            ...staticMember,
            object: renameExpression(staticMember.object),
        };
    }

    if (expression.type === "ConditionalExpression") {
        const conditional: ConditionalExpression = expression;
        return {
            ...conditional,
            test: renameExpression(conditional.test),
            consequent: renameExpression(conditional.consequent),
            alternate: renameExpression(conditional.alternate),
        };
    }

    if (expression.type === "LogicalExpression") {
        const logical: LogicalExpression = expression;
        return {
            ...logical,
            left: renameExpression(logical.left),
            right: renameExpression(logical.right),
        };
    }

    if (expression.type === "BinaryExpression") {
        return {
            ...expression,
            left: renameExpression(expression.left as Expression),
            right: renameExpression(expression.right),
        };
    }

    if (expression.type === "ArrowFunctionExpression") {
        const arrow: ArrowFunctionExpression = expression;
        return { ...arrow, body: renameFunctionBody(arrow.body) };
    }

    if (expression.type === "ObjectExpression") {
        const object: ObjectExpression = expression;
        return { ...object, properties: object.properties.map(renameProperty) };
    }

    if (expression.type === "ArrayExpression") {
        const array: ArrayExpression = expression;
        return {
            ...array,
            elements: array.elements.map(
                (element: ArrayExpressionElement): ArrayExpressionElement => {
                    if (element === null) return element;
                    return renameArgument(element);
                },
            ),
        };
    }

    if (expression.type === "TemplateLiteral") {
        const template: TemplateLiteral = expression;
        return {
            ...template,
            expressions: template.expressions.map(renameExpression),
        };
    }

    if (expression.type === "ParenthesizedExpression") {
        const parenthesized: ParenthesizedExpression = expression;
        return {
            ...parenthesized,
            expression: renameExpression(parenthesized.expression),
        };
    }

    return expression;
};

const renameFunctionBody = (
    body: FunctionBody | Expression,
): FunctionBody | Expression => {
    if (body.type === "BlockStatement") {
        return { ...body, body: body.body.map(renameDirectiveOrStatement) };
    }
    return renameExpression(body);
};

const renameDirectiveOrStatement = (
    node: Directive | Statement,
): Directive | Statement => {
    const renamed: Statement = renameStatement(node as Statement);
    return renamed as Directive | Statement;
};

const renameJsxExpressionContainer = (
    container: JSXExpressionContainer,
): JSXExpressionContainer => {
    if (container.expression.type === "JSXEmptyExpression") return container;
    return { ...container, expression: renameExpression(container.expression) };
};

const renameAttributeItem = (item: JSXAttributeItem): JSXAttributeItem => {
    if (item.type === "JSXSpreadAttribute") {
        return { ...item, argument: renameExpression(item.argument) };
    }

    const attribute: JSXAttribute = item;

    if (attribute.value === null) return attribute;

    if (attribute.value.type === "JSXExpressionContainer") {
        return {
            ...attribute,
            value: renameJsxExpressionContainer(attribute.value),
        };
    }

    if (
        attribute.value.type === "JSXElement" ||
        attribute.value.type === "JSXFragment"
    ) {
        return {
            ...attribute,
            value: renameExpression(attribute.value) as typeof attribute.value,
        };
    }

    return attribute;
};

const renameJsxChild = (child: JSXChild): JSXChild => {
    if (child.type === "JSXElement" || child.type === "JSXFragment") {
        return renameExpression(child) as JSXChild;
    }

    if (child.type === "JSXExpressionContainer") {
        return renameJsxExpressionContainer(child);
    }

    if (child.type === "JSXSpreadChild") {
        return { ...child, expression: renameExpression(child.expression) };
    }

    return child;
};

const renameProperty = (
    property: ObjectProperty | SpreadElement,
): ObjectProperty | SpreadElement => {
    if (property.type === "SpreadElement") {
        const spread: SpreadElement = property;
        return { ...spread, argument: renameExpression(spread.argument) };
    }

    const objectProperty: ObjectProperty = property;

    return {
        ...objectProperty,
        key: objectProperty.computed
            ? (renameExpression(
                  objectProperty.key as Expression,
              ) as typeof objectProperty.key)
            : objectProperty.key,
        value: renameExpression(objectProperty.value),
    };
};

const renameArgument = (argument: Argument): Argument => {
    if (argument.type === "SpreadElement") {
        const spread: SpreadElement = argument;
        return { ...spread, argument: renameExpression(spread.argument) };
    }

    return renameExpression(argument);
};

const renameStatement = (statement: Statement): Statement => {
    if (statement.type === "ExpressionStatement") {
        const node: ExpressionStatement = statement;
        return { ...node, expression: renameExpression(node.expression) };
    }

    if (statement.type === "ReturnStatement") {
        const node: ReturnStatement = statement;
        if (node.argument === null) return node;
        return { ...node, argument: renameExpression(node.argument) };
    }

    if (statement.type === "VariableDeclaration") {
        const node: VariableDeclaration = statement;
        return {
            ...node,
            declarations: node.declarations.map(
                (declarator: VariableDeclarator): VariableDeclarator => {
                    if (declarator.init === null) return declarator;
                    return {
                        ...declarator,
                        init: renameExpression(declarator.init),
                    };
                },
            ),
        };
    }

    if (statement.type === "BlockStatement") {
        const node: BlockStatement = statement;
        return { ...node, body: node.body.map(renameDirectiveOrStatement) };
    }

    if (statement.type === "IfStatement") {
        const node: IfStatement = statement;
        return {
            ...node,
            test: renameExpression(node.test),
            consequent: renameStatement(node.consequent),
            alternate:
                node.alternate === null
                    ? null
                    : renameStatement(node.alternate),
        };
    }

    return statement;
};

const renameIdentifiers = (program: Program): Program => {
    return {
        ...program,
        body: program.body.map(
            (statement: Directive | Statement): Directive | Statement => {
                return renameDirectiveOrStatement(statement);
            },
        ),
    };
};

const transformPlugin: Plugin = {
    name: "rename-jsx-elements",
    transform: (
        _: PluginContext,
        args: TransformArgs,
    ): TransformOutput | null | void => {
        return { ast: renameIdentifiers(args.ast) };
    },
};

export { transformPlugin };
