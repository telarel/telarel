import type { CodeResult } from "#/@types/ast";
import type { CompileOptions } from "#/@types/plugin/options";
import type { RawPlugin } from "#/bridges/plugin";

import { compile as bindingCompile } from "#/binding";
import { toRawPlugins } from "#/bridges/plugin";

/**
 * Compiles a source text with plugins, in memory.
 *
 * Unset options fall back to their defaults (see {@link CompileOptions}).
 */
const compile = async (
    code: string,
    options?: CompileOptions,
): Promise<CodeResult> => {
    const plugins: RawPlugin[] = await toRawPlugins({
        plugins: options?.plugins ?? [],
    });

    const result: Awaited<ReturnType<typeof bindingCompile>> =
        await bindingCompile(code, {
            cwd: options?.cwd,
            file: options?.file,
            language: options?.language,
            sourceType: options?.sourceType,
            sourceMap: options?.sourceMap,
            plugins,
        });

    return { code: result.code, map: result.map ?? null };
};

export { compile };
