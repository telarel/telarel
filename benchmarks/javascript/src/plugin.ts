import type { Plugin, PluginContext, TransformArgs } from "telarel";

import { walk } from "telarel/walker";

const TARGET: string = "Button";

const RENAMED: string = `${TARGET}x`;

const transformPlugin: Plugin = {
    name: "rename-jsx-elements",
    transform: (_: PluginContext, args: TransformArgs): void => {
        walk(args.ast, {
            enter(node): void {
                if (node.type === "JSXIdentifier" && node.name === TARGET) {
                    node.name = RENAMED;
                }

                if (node.type === "Identifier" && node.name === TARGET) {
                    node.name = RENAMED;
                }
            },
        });
    },
};

export { transformPlugin };
