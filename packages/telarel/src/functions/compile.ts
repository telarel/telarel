import type { CodeResult } from "#/@types/ast";
import type { CompileOptions } from "#/@types/plugin/options";
import type { RawPlugin } from "#/bridges/plugin";

import { compile as bindingCompile } from "#/binding";
import { toRawPlugins } from "#/bridges/plugin";
import { CompileError } from "#/errors/compile";

/**
 * Compiles a source text with plugins, in memory.
 *
 * Unset options use their defaults (see {@link CompileOptions}).
 */
const compile = async (
    code: string,
    options?: CompileOptions,
): Promise<CodeResult> => {
    const plugins: RawPlugin[] = await toRawPlugins({
        plugins: options?.plugins ?? [],
    });

    let result: Awaited<ReturnType<typeof bindingCompile>>;

    try {
        result = await bindingCompile(code, {
            cwd: options?.cwd,
            file: options?.file,
            language: options?.language,
            sourceType: options?.sourceType,
            output: options?.output,
            sourceMap: options?.sourceMap,
            plugins,
        });
    } catch (error: unknown) {
        throw CompileError.from(error);
    }

    return { code: result.code, map: result.map ?? null };
};

export { compile };
