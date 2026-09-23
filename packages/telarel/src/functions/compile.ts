import type { BuiltinPlugin } from "#/@types/builtin";
import type { CompileOptions, CompileResult } from "#/@types/compile";
import type { Plugin } from "#/@types/plugin";
import type { RawPlugin } from "#/bridges/plugin";

import { compile as bindingCompile } from "#/binding";
import { toRawPlugin } from "#/bridges/plugin";

const compile = async (options: CompileOptions): Promise<CompileResult> => {
    const metadata: Map<string, unknown> = new Map();

    const plugins: RawPlugin[] =
        options.plugins?.map((plugin: Plugin | BuiltinPlugin): RawPlugin =>
            toRawPlugin(plugin, metadata),
        ) ?? [];

    return await bindingCompile({
        cwd: options.cwd,
        file: options.file,
        code: options.code,
        plugins,
    });
};

export { compile };
