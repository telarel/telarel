import type { CompileOptions, CompileResult } from "#/@types/compile";
import type { RawPlugin } from "#/bridges/plugin";

import { compile as bindingCompile } from "#/binding";
import { toRawPlugins } from "#/bridges/plugin";

/**
 * Compiles a file with plugins, in memory.
 *
 * Unset options fall back to their defaults (see {@link CompileOptions}).
 */
const compile = async (options: CompileOptions): Promise<CompileResult> => {
    const plugins: RawPlugin[] = await toRawPlugins({
        plugins: options.plugins ?? [],
    });

    return await bindingCompile({
        cwd: options.cwd,
        file: options.file,
        code: options.code,
        language: options.language,
        sourceType: options.sourceType,
        plugins,
    });
};

export { compile };
