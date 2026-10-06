import type {
    Plugin,
    PluginContext,
    TransformArgs,
    TransformResult,
} from "telarel";

import { walk } from "telarel/walker";

const NAME: string = "data-telarel";

const VALUE: string = "true";

const jsxAttributePlugin: Plugin = {
    name: "jsx-attribute",
    transform: (_: PluginContext, args: TransformArgs): TransformResult => {
        walk(args.ast, {
            enter(node): void {
                if (node.type !== "JSXOpeningElement") return void 0;

                node.attributes.push({
                    type: "JSXAttribute",
                    name: {
                        type: "JSXIdentifier",
                        name: NAME,
                        start: node.start,
                        end: node.start,
                    },
                    value: {
                        type: "Literal",
                        value: VALUE,
                        raw: `"${VALUE}"`,
                        start: node.start,
                        end: node.start,
                    },
                    start: node.start,
                    end: node.start,
                });
            },
        });

        return { ast: args.ast };
    },
};

export { jsxAttributePlugin };
