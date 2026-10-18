import type { Program } from "@oxc-project/types";

import type { Ast, AstResult } from "#/@types/ast";
import type { ParseOptions } from "#/@types/plugin/options";
import type { RawPlugin } from "#/bridges/plugin";

import { parse as bindingParse } from "#/binding";
import { toRawPlugins } from "#/bridges/plugin";

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

    const result: Awaited<ReturnType<typeof bindingParse>> = await bindingParse(
        code,
        {
            cwd: options?.cwd,
            file: options?.file,
            language: options?.language,
            sourceType: options?.sourceType,
            sourceMap: options?.sourceMap,
            plugins,
        },
    );

    const ast: Ast = {
        program: result.ast.program as Program,
        code: result.ast.code,
        map: result.ast.map,
    };

    return { ast };
};

export { parse };
