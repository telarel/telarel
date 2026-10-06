import type {
    Plugin,
    PluginContext,
    TransformArgs,
    TransformResult,
} from "telarel";

import { walk } from "telarel/walker";

const FROM: string = "Button";

const TO: string = "Pressable";

const componentRenamePlugin: Plugin = {
    name: "component-rename",
    transform: (_: PluginContext, args: TransformArgs): TransformResult => {
        walk(args.ast, {
            enter(node): void {
                if (node.type === "JSXIdentifier" && node.name === FROM) {
                    node.name = TO;
                }

                if (node.type === "Identifier" && node.name === FROM) {
                    node.name = TO;
                }
            },
        });

        return { ast: args.ast };
    },
};

export { componentRenamePlugin };
