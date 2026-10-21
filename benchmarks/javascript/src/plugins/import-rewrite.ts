import type {
    Plugin,
    PluginContext,
    TransformArgs,
    TransformResult,
} from "telarel";

import { walk } from "telarel/walker";

const FROM: string = "react";

const TO: string = "preact";

const importRewritePlugin: Plugin = {
    name: "import-rewrite",
    transform: (_: PluginContext, { ast }: TransformArgs): TransformResult => {
        ast.mutate((program): void => {
            walk(program, {
                enter(node): void {
                    if (node.type === "ImportDeclaration") {
                        if (node.source.value === FROM) {
                            node.source.value = TO;
                        }
                    }
                },
            });
        });

        return { ast };
    },
};

export { importRewritePlugin };
