import type { Program } from "@oxc-project/types";

import type { Ast, AstResult } from "#/@types/ast";
import type { ParserOptions } from "#/@types/parser";

import { builtinParse } from "#/binding";
import { CompileError } from "#/errors/compile";

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
            let ast: Ast;

            try {
                const result: Awaited<ReturnType<typeof builtinParse>> =
                    await builtinParse(code, {
                        cwd: options.cwd,
                        file: options.file,
                        language: options.language,
                        sourceType: options.sourceType,
                        sourceMap: options.sourceMap,
                    });

                ast = {
                    program: result.ast.program as Program,
                    code: result.ast.code,
                    map: result.ast.map,
                };
            } catch (error: unknown) {
                throw CompileError.from(error);
            }

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
