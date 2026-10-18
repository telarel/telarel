import type { Ast, CodeResult } from "#/@types/ast";
import type { GeneratorOptions } from "#/@types/generator";

import { builtinGenerate } from "#/binding";

/**
 * Creates a builtin generator over a fixed AST and options.
 *
 * The returned generator never runs the plugin chain.
 */
const createGenerator = (
    ast: Ast,
    options: GeneratorOptions,
): {
    /**
     * Generates code from the fixed AST with the builtin generator.
     */
    generate: () => Promise<CodeResult>;
} => {
    return {
        generate: async (): Promise<CodeResult> => {
            const result: Awaited<ReturnType<typeof builtinGenerate>> =
                await builtinGenerate(ast, {
                    cwd: options.cwd,
                    file: options.file,
                    sourceMap: options.sourceMap,
                });

            return { code: result.code, map: result.map ?? null };
        },
    };
};

/**
 * A builtin generator built by {@link createGenerator}.
 */
type Generator = ReturnType<typeof createGenerator>;

export type { Generator };
export { createGenerator };
