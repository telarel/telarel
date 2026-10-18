import type { Program } from "@oxc-project/types";

import type { Ast, AstResult } from "#/@types/ast";
import type { ParserOptions } from "#/@types/parser";

import { builtinParse } from "#/binding";

/**
 * Creates a builtin parser over a fixed source text and options.
 *
 * The returned parser never runs the plugin chain.
 */
const createParser = (
    code: string,
    options: ParserOptions,
): {
    /**
     * Parses the fixed source text with the builtin parser.
     */
    parse: () => Promise<AstResult>;
} => {
    return {
        parse: async (): Promise<AstResult> => {
            const result: Awaited<ReturnType<typeof builtinParse>> =
                await builtinParse(code, {
                    cwd: options.cwd,
                    file: options.file,
                    language: options.language,
                    sourceType: options.sourceType,
                    sourceMap: options.sourceMap,
                });

            const ast: Ast = {
                program: result.ast.program as Program,
                code: result.ast.code,
                map: result.ast.map,
            };

            return { ast };
        },
    };
};

/**
 * A builtin parser built by {@link createParser}.
 */
type Parser = ReturnType<typeof createParser>;

export type { Parser };
export { createParser };
