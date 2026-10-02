import type { Program } from "@oxc-project/types";

import type { BuiltinPlugin } from "#/@types/builtin";
import type { Language, SourceType } from "#/@types/grammar";
import type { Plugin } from "#/@types/plugin";
import type {
    CommonPluginContext,
    PluginContext,
    PluginState,
} from "#/@types/plugin/context";
import type { CompileEndArgs } from "#/@types/plugin/hooks/compile-end";
import type { CompileStartArgs } from "#/@types/plugin/hooks/compile-start";
import type { PostResult } from "#/@types/plugin/hooks/post";
import type { PreResult } from "#/@types/plugin/hooks/pre";
import type { TransformResult } from "#/@types/plugin/hooks/transform";
import type { Options, OptionsArgs } from "#/@types/plugin/options";
import type { SourceMap } from "#/@types/sourcemap";

/**
 * Raw plugin context, as carried by every ctx-bearing hook payload: scalar
 * strings with the label convention (empty string = unset for
 * `language`/`sourceType`).
 */
type RawPluginContext = {
    cwd: string;
    file: string;
    code: string;
    language: string;
    sourceType: string;
};

type RawStageArgs = { code: string };

type RawStageOutput = { code: string; map?: string };

type RawTransformArgs = { astJson: string };

type RawCompileStartArgs = {
    cwd: string;
    file: string;
    code: string;
    language: string;
    sourceType: string;
    plugins: Array<string>;
};

type RawCompileEndArgs = {
    code: string;
    map?: string;
    err?: string;
};

type RawOptionsArgs = {
    cwd: string;
    file: string;
    code: string;
    language: string;
    sourceType: string;
    plugins: Array<RawPlugin>;
};

type RawOptionsOutput = {
    cwd?: string;
    file?: string;
    code?: string;
    language?: string;
    sourceType?: string;
    plugins?: Array<RawPlugin>;
};

type RawTransformOutput = { astJson: string } | null;

type RawHookPlugin = {
    name: string;
    options?: (args: RawOptionsArgs) => Promise<RawOptionsOutput | null>;
    compileStart?: (
        ctx: RawPluginContext,
        args: RawCompileStartArgs,
    ) => Promise<void>;
    pre?: (
        ctx: RawPluginContext,
        args: RawStageArgs,
    ) => Promise<RawStageOutput | null>;
    transform?: (
        ctx: RawPluginContext,
        args: RawTransformArgs,
    ) => Promise<RawTransformOutput | null>;
    post?: (
        ctx: RawPluginContext,
        args: RawStageArgs,
    ) => Promise<RawStageOutput | null>;
    compileEnd?: (
        ctx: RawPluginContext,
        args: RawCompileEndArgs,
    ) => Promise<void>;
};

type RawBuiltinPlugin = {
    __builtin: true;
    name: string;
    options?: unknown;
};

type RawPlugin = RawHookPlugin | RawBuiltinPlugin;

// Marks wrapper-produced descriptors, so a descriptor echoed back through an
// options bag (the payload's plugin array roots wrapper outputs) is passed
// through as-is instead of being re-wrapped: re-wrapping would stack the
// translation a second time and corrupt the hook shapes (raw args re-read as
// user args).
const RAW_PLUGIN_MARKER: unique symbol = Symbol("telarel.bridge.rawPlugin");

const isWrappedPlugin = (plugin: Plugin | BuiltinPlugin): boolean =>
    RAW_PLUGIN_MARKER in plugin;

const isBuiltinPlugin = (
    plugin: Plugin | BuiltinPlugin,
): plugin is BuiltinPlugin => {
    return (
        "__builtin" in plugin &&
        (plugin as { __builtin?: unknown }).__builtin === true
    );
};

const toLanguage = (language: string): Language | void => {
    return language.length === 0 ? void 0 : (language as Language);
};

const toSourceType = (sourceType: string): SourceType | void => {
    return sourceType.length === 0 ? void 0 : (sourceType as SourceType);
};

const parseSourceMap = (json: string): SourceMap => {
    return JSON.parse(json) as SourceMap;
};

const toRawStageOutput = (
    result: { code: string; map?: SourceMap | null } | null | void,
): RawStageOutput | null => {
    if (result === null || result === void 0) {
        return null;
    }

    const output: RawStageOutput = { code: result.code };

    if (result.map !== null && result.map !== void 0) {
        output.map = JSON.stringify(result.map);
    }

    return output;
};

type ToRawPluginOptions = {
    plugin: Plugin | BuiltinPlugin;
    state: PluginState;
};

const toRawPlugin = ({ plugin, state }: ToRawPluginOptions): RawPlugin => {
    if (isWrappedPlugin(plugin)) {
        return plugin as unknown as RawPlugin;
    }

    if (typeof plugin.name !== "string" || plugin.name.length === 0) {
        throw new TypeError("plugin.name must be a non-empty string");
    }

    const name: string = plugin.name;

    if (isBuiltinPlugin(plugin)) {
        const builtin: RawBuiltinPlugin = {
            __builtin: true,
            name,
        };

        if ("options" in plugin) {
            builtin.options = plugin.options;
        }

        Object.defineProperty(builtin, RAW_PLUGIN_MARKER, { value: true });

        return builtin;
    }

    const raw: RawHookPlugin = { name };

    Object.defineProperty(raw, RAW_PLUGIN_MARKER, { value: true });

    const toContext = (rawCtx: RawPluginContext): PluginContext => ({
        cwd: rawCtx.cwd,
        module: {
            file: rawCtx.file,
            code: rawCtx.code,
            language: toLanguage(rawCtx.language),
            sourceType: toSourceType(rawCtx.sourceType),
        },
        state,
    });

    const options: Plugin["options"] = plugin.options;

    if (typeof options === "function") {
        raw.options = async (
            rawArgs: RawOptionsArgs,
        ): Promise<RawOptionsOutput | null> => {
            // The payload's plugin descriptors are the bag's current list as
            // raw plugin objects (rooted Rust-side): presented to the bag as
            // `Plugin | BuiltinPlugin` and passed through untouched.
            const bag: Options = {
                cwd: rawArgs.cwd,
                file: rawArgs.file,
                code: rawArgs.code,
                plugins: rawArgs.plugins as unknown as Array<
                    Plugin | BuiltinPlugin
                >,
            };

            if (rawArgs.language.length > 0) {
                bag.language = rawArgs.language as Language;
            }

            if (rawArgs.sourceType.length > 0) {
                bag.sourceType = rawArgs.sourceType as SourceType;
            }

            const ctx: CommonPluginContext = { state };

            const args: OptionsArgs = { options: bag };

            const result: Options | null | void = await options(ctx, args);

            if (result === null || result === void 0) {
                return null;
            }

            const output: RawOptionsOutput = {
                plugins: result.plugins?.map((entry): RawPlugin =>
                    toRawPlugin({ plugin: entry, state }),
                ),
            };

            if (typeof result.cwd === "string") {
                output.cwd = result.cwd;
            }

            if (typeof result.file === "string") {
                output.file = result.file;
            }

            if (typeof result.code === "string") {
                output.code = result.code;
            }

            if (typeof result.language === "string") {
                output.language = result.language;
            }

            if (typeof result.sourceType === "string") {
                output.sourceType = result.sourceType;
            }

            return output;
        };
    }

    const compileStart: Plugin["compileStart"] = plugin.compileStart;

    if (typeof compileStart === "function") {
        raw.compileStart = async (
            rawCtx: RawPluginContext,
            rawArgs: RawCompileStartArgs,
        ): Promise<void> => {
            const args: CompileStartArgs = {
                options: {
                    cwd: rawArgs.cwd,
                    file: rawArgs.file,
                    code: rawArgs.code,
                    language: rawArgs.language as Language,
                    sourceType: rawArgs.sourceType as SourceType,
                    plugins: rawArgs.plugins,
                },
            };

            await compileStart(toContext(rawCtx), args);
        };
    }

    const pre: Plugin["pre"] = plugin.pre;

    if (typeof pre === "function") {
        raw.pre = async (
            rawCtx: RawPluginContext,
            rawArgs: RawStageArgs,
        ): Promise<RawStageOutput | null> => {
            const result: PreResult | null | void = await pre(
                toContext(rawCtx),
                { code: rawArgs.code },
            );

            return toRawStageOutput(result as PreResult | null | void);
        };
    }

    const transform: Plugin["transform"] = plugin.transform;

    if (typeof transform === "function") {
        raw.transform = async (
            rawCtx: RawPluginContext,
            rawArgs: RawTransformArgs,
        ): Promise<RawTransformOutput | null> => {
            const ast: Program = JSON.parse(rawArgs.astJson) as Program;

            const result: TransformResult | null | void = await transform(
                toContext(rawCtx),
                { ast },
            );

            if (result === null || result === void 0) {
                return null;
            }

            return { astJson: JSON.stringify(result.ast) };
        };
    }

    const post: Plugin["post"] = plugin.post;

    if (typeof post === "function") {
        raw.post = async (
            rawCtx: RawPluginContext,
            rawArgs: RawStageArgs,
        ): Promise<RawStageOutput | null> => {
            const result: PostResult | null | void = await post(
                toContext(rawCtx),
                { code: rawArgs.code },
            );

            return toRawStageOutput(result as PostResult | null | void);
        };
    }

    const compileEnd: Plugin["compileEnd"] = plugin.compileEnd;

    if (typeof compileEnd === "function") {
        raw.compileEnd = async (
            rawCtx: RawPluginContext,
            rawArgs: RawCompileEndArgs,
        ): Promise<void> => {
            const args: CompileEndArgs = {
                code: rawArgs.code,
                map:
                    rawArgs.map === void 0 ? null : parseSourceMap(rawArgs.map),
            };

            if (rawArgs.err !== void 0) {
                args.err = new Error(rawArgs.err);
            }

            await compileEnd(toContext(rawCtx), args);
        };
    }

    return raw;
};

export type { RawPlugin };
export { toRawPlugin };
