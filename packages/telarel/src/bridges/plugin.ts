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
import type {
    FinalizeArgs,
    FinalizeResult,
} from "#/@types/plugin/hooks/finalize";
import type { PrepareArgs, PrepareResult } from "#/@types/plugin/hooks/prepare";
import type {
    TransformArgs,
    TransformResult,
} from "#/@types/plugin/hooks/transform";
import type {
    Options,
    OptionsArgs,
    PluginOption,
} from "#/@types/plugin/options";
import type { SourceMap } from "#/@types/sourcemap";
import type { NormalizedHook } from "#/bridges/normalize-hook";

import { normalizeHook } from "#/bridges/normalize-hook";

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

type RawHookMeta = { order?: "pre" | "post" };

type RawHookPlugin = {
    name: string;
    options?: (args: RawOptionsArgs) => Promise<RawOptionsOutput | null>;
    optionsMeta?: RawHookMeta;
    compileStart?: (
        ctx: RawPluginContext,
        args: RawCompileStartArgs,
    ) => Promise<void>;
    compileStartMeta?: RawHookMeta;
    prepare?: (
        ctx: RawPluginContext,
        args: RawStageArgs,
    ) => Promise<RawStageOutput | null>;
    prepareMeta?: RawHookMeta;
    transform?: (
        ctx: RawPluginContext,
        args: RawTransformArgs,
    ) => Promise<RawTransformOutput | null>;
    transformMeta?: RawHookMeta;
    finalize?: (
        ctx: RawPluginContext,
        args: RawStageArgs,
    ) => Promise<RawStageOutput | null>;
    finalizeMeta?: RawHookMeta;
    compileEnd?: (
        ctx: RawPluginContext,
        args: RawCompileEndArgs,
    ) => Promise<void>;
    compileEndMeta?: RawHookMeta;
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

type ToRawPluginsOptions = {
    plugins: ReadonlyArray<PluginOption>;
    state: PluginState;
};

type OptionsHook = (
    ctx: CommonPluginContext,
    args: OptionsArgs,
) => Options | null | void | Promise<Options | null | void>;

type CompileStartHook = (
    ctx: PluginContext,
    args: CompileStartArgs,
) => void | Promise<void>;

type PrepareHook = (
    ctx: PluginContext,
    args: PrepareArgs,
) => PrepareResult | null | void | Promise<PrepareResult | null | void>;

type TransformHook = (
    ctx: PluginContext,
    args: TransformArgs,
) => TransformResult | null | void | Promise<TransformResult | null | void>;

type FinalizeHook = (
    ctx: PluginContext,
    args: FinalizeArgs,
) => FinalizeResult | null | void | Promise<FinalizeResult | null | void>;

type CompileEndHook = (
    ctx: PluginContext,
    args: CompileEndArgs,
) => void | Promise<void>;

/**
 * Flatten a plugin option list into concrete plugins.
 *
 * Nested arrays recurse, promises await, and `false`/`null`/`undefined` entries
 * are skipped. Plugin objects pass through unchanged.
 */
const flattenPlugins = async (
    options: ReadonlyArray<PluginOption>,
): Promise<Array<Plugin | BuiltinPlugin>> => {
    const flattened: Array<Plugin | BuiltinPlugin> = [];

    for (const option of options) {
        const resolved: Awaited<PluginOption> = await option;

        if (resolved === false || resolved === null || resolved === void 0) {
            continue;
        }

        if (Array.isArray(resolved)) {
            flattened.push(...(await flattenPlugins(resolved)));
            continue;
        }

        flattened.push(resolved);
    }

    return flattened;
};

const toRawPlugin = ({ plugin, state }: ToRawPluginOptions): RawPlugin => {
    if (typeof plugin === "function" || Array.isArray(plugin)) {
        throw new TypeError(
            "plugin must be an object with a `name`; pass a plugin object, not a factory",
        );
    }

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

    const options: NormalizedHook<OptionsHook> = normalizeHook<OptionsHook>(
        plugin.options,
    );

    if (typeof options.handler === "function") {
        const optionsHandler: OptionsHook = options.handler;

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
                plugins: rawArgs.plugins as unknown as Array<PluginOption>,
            };

            if (rawArgs.language.length > 0) {
                bag.language = rawArgs.language as Language;
            }

            if (rawArgs.sourceType.length > 0) {
                bag.sourceType = rawArgs.sourceType as SourceType;
            }

            const ctx: CommonPluginContext = { state };

            const args: OptionsArgs = { options: bag };

            const result: Options | null | void = await optionsHandler(
                ctx,
                args,
            );

            if (result === null || result === void 0) {
                return null;
            }

            const output: RawOptionsOutput = {
                plugins: await toRawPlugins({
                    plugins: result.plugins ?? [],
                    state,
                }),
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

        if (options.meta.order !== void 0) {
            raw.optionsMeta = { order: options.meta.order };
        }
    }

    const compileStart: NormalizedHook<CompileStartHook> =
        normalizeHook<CompileStartHook>(plugin.compileStart);

    if (typeof compileStart.handler === "function") {
        const compileStartHandler: CompileStartHook = compileStart.handler;

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

            await compileStartHandler(toContext(rawCtx), args);
        };

        if (compileStart.meta.order !== void 0) {
            raw.compileStartMeta = { order: compileStart.meta.order };
        }
    }

    const prepare: NormalizedHook<PrepareHook> = normalizeHook<PrepareHook>(
        plugin.prepare,
    );

    if (typeof prepare.handler === "function") {
        const prepareHandler: PrepareHook = prepare.handler;

        raw.prepare = async (
            rawCtx: RawPluginContext,
            rawArgs: RawStageArgs,
        ): Promise<RawStageOutput | null> => {
            const result: PrepareResult | null | void = await prepareHandler(
                toContext(rawCtx),
                { code: rawArgs.code },
            );

            return toRawStageOutput(result as PrepareResult | null | void);
        };

        if (prepare.meta.order !== void 0) {
            raw.prepareMeta = { order: prepare.meta.order };
        }
    }

    const transform: NormalizedHook<TransformHook> =
        normalizeHook<TransformHook>(plugin.transform);

    if (typeof transform.handler === "function") {
        const transformHandler: TransformHook = transform.handler;

        raw.transform = async (
            rawCtx: RawPluginContext,
            rawArgs: RawTransformArgs,
        ): Promise<RawTransformOutput | null> => {
            const ast: Program = JSON.parse(rawArgs.astJson) as Program;

            const result: TransformResult | null | void =
                await transformHandler(toContext(rawCtx), { ast });

            if (result === null || result === void 0) {
                return null;
            }

            return { astJson: JSON.stringify(result.ast) };
        };

        if (transform.meta.order !== void 0) {
            raw.transformMeta = { order: transform.meta.order };
        }
    }

    const finalize: NormalizedHook<FinalizeHook> = normalizeHook<FinalizeHook>(
        plugin.finalize,
    );

    if (typeof finalize.handler === "function") {
        const finalizeHandler: FinalizeHook = finalize.handler;

        raw.finalize = async (
            rawCtx: RawPluginContext,
            rawArgs: RawStageArgs,
        ): Promise<RawStageOutput | null> => {
            const result: FinalizeResult | null | void = await finalizeHandler(
                toContext(rawCtx),
                { code: rawArgs.code },
            );

            return toRawStageOutput(result as FinalizeResult | null | void);
        };

        if (finalize.meta.order !== void 0) {
            raw.finalizeMeta = { order: finalize.meta.order };
        }
    }

    const compileEnd: NormalizedHook<CompileEndHook> =
        normalizeHook<CompileEndHook>(plugin.compileEnd);

    if (typeof compileEnd.handler === "function") {
        const compileEndHandler: CompileEndHook = compileEnd.handler;

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

            await compileEndHandler(toContext(rawCtx), args);
        };

        if (compileEnd.meta.order !== void 0) {
            raw.compileEndMeta = { order: compileEnd.meta.order };
        }
    }

    return raw;
};

const toRawPlugins = async ({
    plugins,
    state,
}: ToRawPluginsOptions): Promise<RawPlugin[]> => {
    const flattened: Array<Plugin | BuiltinPlugin> =
        await flattenPlugins(plugins);

    return flattened.map((plugin: Plugin | BuiltinPlugin): RawPlugin =>
        toRawPlugin({ plugin, state }),
    );
};

export type { RawPlugin };
export { toRawPlugin, toRawPlugins };
