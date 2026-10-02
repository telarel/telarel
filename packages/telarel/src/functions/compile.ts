import type { BuiltinPlugin } from "#/@types/builtin";
import type { CompileOptions, CompileResult } from "#/@types/compile";
import type { Plugin } from "#/@types/plugin";
import type { PluginState } from "#/@types/plugin/context";
import type { RawPlugin } from "#/bridges/plugin";

import { compile as bindingCompile } from "#/binding";
import { toRawPlugin } from "#/bridges/plugin";

const compile = async (options: CompileOptions): Promise<CompileResult> => {
    const state: PluginState = new Map();

    const plugins: RawPlugin[] =
        options.plugins?.map((plugin: Plugin | BuiltinPlugin): RawPlugin =>
            toRawPlugin({ plugin, state }),
        ) ?? [];

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
