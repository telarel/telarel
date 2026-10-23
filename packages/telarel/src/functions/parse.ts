import type { Program } from "@oxc-project/types";

import type { AstResult } from "#/@types/ast";
import type { Language } from "#/@types/grammar";
import type { ParseOptions } from "#/@types/plugin/options";
import type { RawPlugin } from "#/bridges/plugin";

import { parse as bindingParse } from "#/binding";
import { toRawPlugins } from "#/bridges/plugin";
import { Ast } from "#/classes/ast";
import { CompileError } from "#/errors/compile";

/**
 * Parses a source text with plugins, in memory.
 *
 * Unset options fall back to their defaults (see {@link ParseOptions}).
 */
const parse = async (
    code: string,
    options?: ParseOptions,
): Promise<AstResult> => {
    const plugins: RawPlugin[] = await toRawPlugins({
        plugins: options?.plugins ?? [],
    });

    let ast: Ast;

    try {
        const result: Awaited<ReturnType<typeof bindingParse>> =
            await bindingParse(code, {
                cwd: options?.cwd,
                file: options?.file,
                language: options?.language,
                sourceType: options?.sourceType,
                inputSourceMap: options?.inputSourceMap,
                plugins,
            });

        ast = new Ast({
            program: result.ast.program as Program,
            source: result.ast.code,
            language: result.ast.language as Language,
            map: result.ast.map,
        });
    } catch (error: unknown) {
        throw CompileError.from(error);
    }

    return { ast };
};

export { parse };
