import type { CodeResult } from "#/@types/ast";
import type { GeneratorOptions } from "#/@types/generator";
import type { Ast } from "#/classes/ast";

import { builtinGenerate } from "#/binding";
import { toAstInput } from "#/bridges/plugin";
import { CompileError } from "#/errors/compile";

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
            let result: Awaited<ReturnType<typeof builtinGenerate>>;

            try {
                result = await builtinGenerate(toAstInput(ast), {
                    cwd: options.cwd,
                    file: options.file,
                    sourceMap: options.sourceMap,
                });
            } catch (error: unknown) {
                throw CompileError.from(error);
            }

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
