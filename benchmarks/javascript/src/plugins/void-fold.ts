import type {
    Plugin,
    PluginContext,
    TransformArgs,
    TransformResult,
} from "telarel";
import type { WalkerThisContextEnter } from "telarel/walker";

import { walk } from "telarel/walker";

const NAME: string = "undefined";

const voidFoldPlugin: Plugin = {
    name: "void-fold",
    transform: (_: PluginContext, { ast }: TransformArgs): TransformResult => {
        ast.mutate((program): void => {
            walk(program, {
                enter(this: WalkerThisContextEnter, node): void {
                    if (node.type !== "UnaryExpression") return void 0;
                    if (node.operator !== "void") return void 0;
                    if (node.argument.type !== "Literal") return void 0;
                    if (node.argument.value !== 0) return void 0;

                    this.replace({
                        type: "Identifier",
                        name: NAME,
                        start: node.start,
                        end: node.end,
                    });
                },
            });
        });

        return { ast };
    },
};

export { voidFoldPlugin };
