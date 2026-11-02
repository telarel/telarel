import type { AstResult, CodeResult } from "#/@types/ast";
import type { BuiltinPlugin } from "#/@types/builtin";
import type { GeneratorOptions } from "#/@types/generator";
import type { Language } from "#/@types/grammar";
import type { ParserOptions } from "#/@types/parser";
import type { Plugin } from "#/@types/plugin";
import type {
    CommonPluginContext,
    PluginContext,
} from "#/@types/plugin/context";
import type { EndArgs } from "#/@types/plugin/hooks/end";
import type { FinalizeArgs } from "#/@types/plugin/hooks/finalize";
import type { GenerateArgs } from "#/@types/plugin/hooks/generate";
import type { ParseArgs } from "#/@types/plugin/hooks/parse";
import type { PrepareArgs } from "#/@types/plugin/hooks/prepare";
import type { StartArgs } from "#/@types/plugin/hooks/start";
import type {
    TransformArgs,
    TransformResult,
} from "#/@types/plugin/hooks/transform";
import type {
    CompleteOutputOptions,
    OptionsArgs,
    OutputOptions,
    PluginOption,
    ResolvableOptions,
} from "#/@types/plugin/options";
import type { Program } from "#/@types/program";
import type { SourceMapOptions } from "#/@types/sourcemap";
import type { SourceMap } from "#/@types/sourcemap/map";
import type { NormalizedHook } from "#/bridges/normalize-hook";

import { parseLanguage, parseSourceType } from "#/bridges/grammar";
import { normalizeHook } from "#/bridges/normalize-hook";
import { AST_MUTATED, AST_RESET_MUTATED, Ast } from "#/classes/ast";
import { getPluginErrorMessage, PluginError } from "#/errors/plugin";
import { encodePluginError } from "#/errors/transport/plugin-error";

/**
 * Raw plugin context, as carried by every ctx-bearing hook payload. The binding
 * resolves `language` and `sourceType`. The bridge parses both labels.
 */
type RawPluginContext = {
    cwd: string;
    file: string;
    language: string;
    sourceType: string;
};

type RawAstArgs = {
    astJson: string;
    code: string;
    map?: SourceMap | null;
    language?: string;
};

type AstInput = {
    program: Program;
    code: string;
    map?: SourceMap;
    language?: Language;
};

type RawTransformAstOutput = {
    astJson: string;
    code: string;
    map?: SourceMap;
    language?: string;
};

type RawTransformOutput = RawTransformAstOutput | null;

type RawOptionsArgs = {
    cwd?: string;
    file?: string;
    language: string;
    sourceType: string;
    output?: OutputOptions;
    sourceMap?: SourceMapOptions;
    plugins: Array<RawPlugin>;
};

type RawOptionsOutput = {
    cwd?: string;
    file?: string;
    language?: string;
    sourceType?: string;
    output?: OutputOptions;
    sourceMap?: SourceMapOptions;
    plugins?: Array<RawPlugin>;
};

type RawStartArgs = {
    options: {
        cwd: string;
        file: string;
        language: string;
        sourceType: string;
        output: CompleteOutputOptions;
        sourceMap: SourceMapOptions;
        plugins: Array<string>;
    };
};

type RawParserOptions = {
    cwd: string;
    file: string;
    language: string;
    sourceType: string;
    sourceMap: SourceMapOptions;
};

type RawParseArgs = {
    code: string;
    options: RawParserOptions;
};

type RawParseOutput = { ast: AstInput } | null;

type RawGeneratorOptions = {
    cwd: string;
    file: string;
    sourceMap: SourceMapOptions;
};

type RawGenerateArgs = {
    ast: AstInput;
    options: RawGeneratorOptions;
};

type RawCodeOutput = { code: string; map?: SourceMap } | null;

type RawEndArgs = { error?: string };

type RawHookMeta = { order?: "pre" | "post" };

type RawHookPlugin = {
    name: string;
    options?: (args: RawOptionsArgs) => Promise<RawOptionsOutput | null>;
    optionsMeta?: RawHookMeta;
    start?: (ctx: RawPluginContext, args: RawStartArgs) => Promise<void>;
    startMeta?: RawHookMeta;
    parse?: (
        ctx: RawPluginContext,
        args: RawParseArgs,
    ) => Promise<RawParseOutput>;
    parseMeta?: RawHookMeta;
    prepare?: (ctx: RawPluginContext, args: RawAstArgs) => Promise<void>;
    prepareMeta?: RawHookMeta;
    transform?: (
        ctx: RawPluginContext,
        args: RawAstArgs,
    ) => Promise<RawTransformOutput | null>;
    transformMeta?: RawHookMeta;
    finalize?: (ctx: RawPluginContext, args: RawAstArgs) => Promise<void>;
    finalizeMeta?: RawHookMeta;
    generate?: (
        ctx: RawPluginContext,
        args: RawGenerateArgs,
    ) => Promise<RawCodeOutput>;
    generateMeta?: RawHookMeta;
    end?: (ctx: RawPluginContext, args: RawEndArgs) => Promise<void>;
    endMeta?: RawHookMeta;
};

type RawBuiltinPlugin = {
    __builtin: true;
    name: string;
    options?: unknown;
};

type RawPlugin = RawHookPlugin | RawBuiltinPlugin;

type RawTransformBatchArgs = RawAstArgs & {
    plugins: Array<RawPlugin>;
};

// Marks wrapper-produced descriptors so a descriptor echoed back through an
// options bag is passed through instead of being translated a second time.
const RAW_PLUGIN_MARKER: unique symbol = Symbol("telarel.bridge.rawPlugin");

const isWrappedPlugin = (plugin: Plugin | BuiltinPlugin): boolean =>
    RAW_PLUGIN_MARKER in plugin;

// Must match `PLUGIN_ID` in `crates/binding/src/plugin/build.rs`.
const PLUGIN_ID: string = "__PLUGIN_ID__";

// Must match `TRANSFORM_BATCH_KEY` in `crates/binding/src/plugin/build.rs`.
const TRANSFORM_BATCH: string = "__TRANSFORM_BATCH__";

// Read only by the batch runner below. The binding never scans this key.
const TRANSFORM_ENTRY: string = "__TRANSFORM_ENTRY__";

// Batching is default-on. `TELAREL_TRANSFORM_BATCH=0` is an internal test and
// diagnostic opt-out that forces the per-plugin path.
const isTransformBatchEnabled = (): boolean =>
    process.env.TELAREL_TRANSFORM_BATCH !== "0";

// Internal diagnostic trace, not public API. `1` enables it.
const TRANSFORM_BATCH_DEBUG: string = "TELAREL_TRANSFORM_BATCH_DEBUG";

const pluginIds: WeakMap<Plugin | BuiltinPlugin, number> = new WeakMap();

let nextPluginId: number = 1;

const tagPluginId = (
    raw: RawPlugin,
    plugin: Plugin | BuiltinPlugin,
): RawPlugin => {
    let id: number | undefined = pluginIds.get(plugin);

    if (id === void 0) {
        id = nextPluginId++;
        pluginIds.set(plugin, id);
    }

    Object.defineProperty(raw, PLUGIN_ID, { value: id, enumerable: false });

    return raw;
};

const isBuiltinPlugin = (
    plugin: Plugin | BuiltinPlugin,
): plugin is BuiltinPlugin => {
    return (
        "__builtin" in plugin &&
        (plugin as { __builtin?: unknown }).__builtin === true
    );
};

const toLanguage = (language: string): Language | void => {
    return language.length === 0 ? void 0 : parseLanguage(language);
};

const toContext = (rawCtx: RawPluginContext): PluginContext => ({
    cwd: rawCtx.cwd,
    module: {
        file: rawCtx.file,
        language: parseLanguage(rawCtx.language),
        sourceType: parseSourceType(rawCtx.sourceType),
    },
});

const toParserOptions = (rawOptions: RawParserOptions): ParserOptions => ({
    cwd: rawOptions.cwd,
    file: rawOptions.file,
    language: parseLanguage(rawOptions.language),
    sourceType: parseSourceType(rawOptions.sourceType),
    sourceMap: rawOptions.sourceMap,
});

const toGeneratorOptions = (
    rawOptions: RawGeneratorOptions,
): GeneratorOptions => ({
    cwd: rawOptions.cwd,
    file: rawOptions.file,
    sourceMap: rawOptions.sourceMap,
});

type ToRawPluginOptions = {
    plugin: Plugin | BuiltinPlugin;
};

type ToRawPluginsOptions = {
    plugins: ReadonlyArray<PluginOption>;
};

type OptionsHook = (
    ctx: CommonPluginContext,
    args: OptionsArgs,
) => OptionsArgs | null | void | Promise<OptionsArgs | null | void>;

type StartHook = (ctx: PluginContext, args: StartArgs) => void | Promise<void>;

type ParseHook = (
    ctx: PluginContext,
    args: ParseArgs,
) => AstResult | null | void | Promise<AstResult | null | void>;

type PrepareHook = (
    ctx: PluginContext,
    args: PrepareArgs,
) => void | Promise<void>;

type TransformHook = (
    ctx: PluginContext,
    args: TransformArgs,
) => TransformResult | null | void | Promise<TransformResult | null | void>;

type FinalizeHook = (
    ctx: PluginContext,
    args: FinalizeArgs,
) => void | Promise<void>;

type GenerateHook = (
    ctx: PluginContext,
    args: GenerateArgs,
) => CodeResult | null | void | Promise<CodeResult | null | void>;

type EndHook = (ctx: PluginContext, args: EndArgs) => void | Promise<void>;

const runHook = async <Result>(
    body: () => Promise<Result>,
): Promise<Result> => {
    try {
        return await body();
    } catch (error: unknown) {
        if (error instanceof PluginError) {
            throw encodePluginError({
                message: getPluginErrorMessage(error),
                source: error.source,
            });
        }

        throw error;
    }
};

const isAst = (value: unknown): value is Ast => value instanceof Ast;

const fromAstInput = (ast: AstInput): Ast =>
    new Ast({
        program: ast.program,
        source: ast.code,
        language: ast.language,
        map: ast.map,
    });

const toAstInput = (ast: Ast): AstInput => {
    const input: AstInput = {
        program: ast.program as Program,
        code: ast.source,
        map: (ast.map as SourceMap | null) ?? void 0,
    };

    if (typeof ast.language === "string") {
        input.language = ast.language;
    }

    return input;
};

const toAst = ({ args }: { args: RawAstArgs }): Ast =>
    fromAstInput({
        program: JSON.parse(args.astJson) as Program,
        code: args.code,
        language: toLanguage(args.language ?? "") ?? void 0,
        map: args.map ?? void 0,
    });

const requireAst = (value: unknown): Ast => {
    if (!isAst(value)) {
        throw new TypeError(
            "Expected an Ast instance; use new Ast({ program, source })",
        );
    }

    return value;
};

const toRawTransformOutput = (ast: Ast): RawTransformAstOutput => {
    const input: AstInput = toAstInput(ast);

    const output: RawTransformAstOutput = {
        astJson: JSON.stringify(input.program),
        code: input.code,
    };

    if (input.map !== void 0) {
        output.map = input.map;
    }

    if (typeof input.language === "string") {
        output.language = input.language;
    }

    return output;
};

const isTransformNoOp = (current: Ast, next: Ast): boolean =>
    next === current &&
    !current[AST_MUTATED]() &&
    (current.program.comments?.length ?? 0) === 0;

const normalizeReturnedAst = (next: Ast, previous: Ast): Ast => {
    const language: Language | void = next.language;

    const effective: Language | void =
        typeof language === "string" && language.length > 0
            ? language
            : previous.language;

    if (effective === next.language) {
        return next;
    }

    return new Ast({
        program: next.program as Program,
        source: next.source,
        language: typeof effective === "string" ? effective : void 0,
        map: (next.map as SourceMap | null) ?? void 0,
    });
};

type CallTransformEntryOptions = {
    entry: TransformHook;
    rawCtx: RawPluginContext;
    ast: Ast;
};

const callTransformEntry = async (
    options: CallTransformEntryOptions,
): Promise<TransformResult | null> =>
    await runHook(async (): Promise<TransformResult | null> => {
        const value: TransformResult | null | void = await options.entry(
            toContext(options.rawCtx),
            { ast: options.ast },
        );

        if (value === null || value === void 0) {
            return null;
        }

        return { ast: requireAst(value.ast) };
    });

const runTransformBatch = async (
    rawCtx: RawPluginContext,
    rawArgs: RawTransformBatchArgs,
): Promise<RawTransformOutput> =>
    await runHook(async (): Promise<RawTransformOutput> => {
        let currentAst: Ast = toAst({ args: rawArgs });

        // The checkpoint shares `map`. In-place `map` mutation is unsupported in a batch.
        let checkpoint: RawTransformAstOutput = {
            astJson: rawArgs.astJson,
            code: rawArgs.code,
        };

        if (rawArgs.map !== void 0 && rawArgs.map !== null) {
            checkpoint.map = rawArgs.map;
        }

        if (
            typeof rawArgs.language === "string" &&
            rawArgs.language.length > 0
        ) {
            checkpoint.language = rawArgs.language;
        }

        let changed: boolean = false;

        if (process.env[TRANSFORM_BATCH_DEBUG] === "1") {
            console.debug("[telarel] transform batch", {
                plugins: rawArgs.plugins.map(
                    (plugin: RawPlugin): string => plugin.name,
                ),
            });
        }

        for (const plugin of rawArgs.plugins) {
            const entry: unknown = (plugin as Record<string, unknown>)[
                TRANSFORM_ENTRY
            ];

            if (typeof entry !== "function") {
                throw new TypeError(
                    `transform batch member is missing ${TRANSFORM_ENTRY}`,
                );
            }

            // The mutation marker is sticky across calls, so clear it before
            // each member to attribute only this member's mutation.
            currentAst[AST_RESET_MUTATED]();

            const result: TransformResult | null = await callTransformEntry({
                entry: entry as TransformHook,
                rawCtx,
                ast: currentAst,
            });

            if (result === null) {
                if (currentAst[AST_MUTATED]()) {
                    // Discard the in-place mutation instead of serializing it.
                    currentAst = toAst({ args: checkpoint });
                }

                continue;
            }

            const next: Ast = result.ast;

            if (isTransformNoOp(currentAst, next)) {
                continue;
            }

            changed = true;

            if (next !== currentAst) {
                currentAst = normalizeReturnedAst(next, currentAst);
            }

            checkpoint = toRawTransformOutput(currentAst);
        }

        return changed ? checkpoint : null;
    });

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

const toRawPlugin = ({ plugin }: ToRawPluginOptions): RawPlugin => {
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

        return tagPluginId(builtin, plugin);
    }

    const raw: RawHookPlugin = { name };

    Object.defineProperty(raw, RAW_PLUGIN_MARKER, { value: true });

    const options: NormalizedHook<OptionsHook> = normalizeHook<OptionsHook>(
        plugin.options,
    );

    if (typeof options.handler === "function") {
        const optionsHandler: OptionsHook = options.handler;

        raw.options = async (
            rawArgs: RawOptionsArgs,
        ): Promise<RawOptionsOutput | null> =>
            await runHook(async (): Promise<RawOptionsOutput | null> => {
                // The payload's descriptors are already raw plugin objects.
                const bag: ResolvableOptions = {
                    plugins: rawArgs.plugins as unknown as Array<PluginOption>,
                };

                // An unset `cwd` or `file` stays absent so an echoed bag
                // keeps the defaults. An empty `language` or `sourceType`
                // label also means unset.
                if (typeof rawArgs.cwd === "string") {
                    bag.cwd = rawArgs.cwd;
                }

                if (typeof rawArgs.file === "string") {
                    bag.file = rawArgs.file;
                }

                if (rawArgs.language.length > 0) {
                    bag.language = parseLanguage(rawArgs.language);
                }

                if (rawArgs.sourceType.length > 0) {
                    bag.sourceType = parseSourceType(rawArgs.sourceType);
                }

                if (rawArgs.output !== void 0) {
                    bag.output = rawArgs.output;
                }

                if (rawArgs.sourceMap !== void 0) {
                    bag.sourceMap = rawArgs.sourceMap;
                }

                const ctx: CommonPluginContext = {};

                const args: OptionsArgs = { options: bag };

                const result: OptionsArgs | null | void = await optionsHandler(
                    ctx,
                    args,
                );

                if (result === null || result === void 0) {
                    return null;
                }

                const next: ResolvableOptions = result.options;

                const output: RawOptionsOutput = {
                    plugins: await toRawPlugins({
                        plugins: next.plugins ?? [],
                    }),
                };

                if (typeof next.cwd === "string") {
                    output.cwd = next.cwd;
                }

                if (typeof next.file === "string") {
                    output.file = next.file;
                }

                if (typeof next.language === "string") {
                    output.language = next.language;
                }

                if (typeof next.sourceType === "string") {
                    output.sourceType = next.sourceType;
                }

                if (next.output !== void 0) {
                    output.output = next.output;
                }

                if (next.sourceMap !== void 0) {
                    output.sourceMap = next.sourceMap;
                }

                return output;
            });

        if (options.meta.order !== void 0) {
            raw.optionsMeta = { order: options.meta.order };
        }
    }

    const start: NormalizedHook<StartHook> = normalizeHook<StartHook>(
        plugin.start,
    );

    if (typeof start.handler === "function") {
        const startHandler: StartHook = start.handler;

        raw.start = async (
            rawCtx: RawPluginContext,
            rawArgs: RawStartArgs,
        ): Promise<void> =>
            await runHook(async (): Promise<void> => {
                const args: StartArgs = {
                    options: {
                        cwd: rawArgs.options.cwd,
                        file: rawArgs.options.file,
                        language: parseLanguage(rawArgs.options.language),
                        sourceType: parseSourceType(rawArgs.options.sourceType),
                        output: rawArgs.options.output,
                        sourceMap: rawArgs.options.sourceMap,
                        plugins: rawArgs.options.plugins,
                    },
                };

                await startHandler(toContext(rawCtx), args);
            });

        if (start.meta.order !== void 0) {
            raw.startMeta = { order: start.meta.order };
        }
    }

    const parse: NormalizedHook<ParseHook> = normalizeHook<ParseHook>(
        plugin.parse,
    );

    if (typeof parse.handler === "function") {
        const parseHandler: ParseHook = parse.handler;

        raw.parse = async (
            rawCtx: RawPluginContext,
            rawArgs: RawParseArgs,
        ): Promise<RawParseOutput> =>
            await runHook(async (): Promise<RawParseOutput> => {
                const args: ParseArgs = {
                    code: rawArgs.code,
                    options: toParserOptions(rawArgs.options),
                };

                const result: AstResult | null | void = await parseHandler(
                    toContext(rawCtx),
                    args,
                );

                if (result === null || result === void 0) {
                    return null;
                }

                return { ast: toAstInput(requireAst(result.ast)) };
            });

        if (parse.meta.order !== void 0) {
            raw.parseMeta = { order: parse.meta.order };
        }
    }

    const prepare: NormalizedHook<PrepareHook> = normalizeHook<PrepareHook>(
        plugin.prepare,
    );

    if (typeof prepare.handler === "function") {
        const prepareHandler: PrepareHook = prepare.handler;

        raw.prepare = async (
            rawCtx: RawPluginContext,
            rawArgs: RawAstArgs,
        ): Promise<void> =>
            await runHook(async (): Promise<void> => {
                await prepareHandler(toContext(rawCtx), {
                    ast: toAst({ args: rawArgs }),
                });
            });

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
            rawArgs: RawAstArgs,
        ): Promise<RawTransformOutput | null> =>
            await runHook(async (): Promise<RawTransformOutput | null> => {
                const ast: Ast = toAst({ args: rawArgs });

                const result: TransformResult | null | void =
                    await transformHandler(toContext(rawCtx), { ast });

                if (result === null || result === void 0) {
                    return null;
                }

                const next: Ast = requireAst(result.ast);

                if (isTransformNoOp(ast, next)) {
                    return null;
                }

                return toRawTransformOutput(next);
            });

        Object.defineProperty(raw, TRANSFORM_ENTRY, {
            value: transformHandler,
        });

        if (isTransformBatchEnabled()) {
            Object.defineProperty(raw, TRANSFORM_BATCH, {
                value: runTransformBatch,
            });
        }

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
            rawArgs: RawAstArgs,
        ): Promise<void> =>
            await runHook(async (): Promise<void> => {
                await finalizeHandler(toContext(rawCtx), {
                    ast: toAst({ args: rawArgs }),
                });
            });

        if (finalize.meta.order !== void 0) {
            raw.finalizeMeta = { order: finalize.meta.order };
        }
    }

    const generate: NormalizedHook<GenerateHook> = normalizeHook<GenerateHook>(
        plugin.generate,
    );

    if (typeof generate.handler === "function") {
        const generateHandler: GenerateHook = generate.handler;

        raw.generate = async (
            rawCtx: RawPluginContext,
            rawArgs: RawGenerateArgs,
        ): Promise<RawCodeOutput> =>
            await runHook(async (): Promise<RawCodeOutput> => {
                const args: GenerateArgs = {
                    ast: fromAstInput(rawArgs.ast),
                    options: toGeneratorOptions(rawArgs.options),
                };

                const result: CodeResult | null | void = await generateHandler(
                    toContext(rawCtx),
                    args,
                );

                if (result === null || result === void 0) {
                    return null;
                }

                const output: RawCodeOutput = { code: result.code };

                if (result.map !== void 0 && result.map !== null) {
                    output.map = result.map;
                }

                return output;
            });

        if (generate.meta.order !== void 0) {
            raw.generateMeta = { order: generate.meta.order };
        }
    }

    const end: NormalizedHook<EndHook> = normalizeHook<EndHook>(plugin.end);

    if (typeof end.handler === "function") {
        const endHandler: EndHook = end.handler;

        raw.end = async (
            rawCtx: RawPluginContext,
            rawArgs: RawEndArgs,
        ): Promise<void> =>
            await runHook(async (): Promise<void> => {
                const args: EndArgs = { error: rawArgs.error ?? null };

                await endHandler(toContext(rawCtx), args);
            });

        if (end.meta.order !== void 0) {
            raw.endMeta = { order: end.meta.order };
        }
    }

    return tagPluginId(raw, plugin);
};

const toRawPlugins = async ({
    plugins,
}: ToRawPluginsOptions): Promise<RawPlugin[]> => {
    const flattened: Array<Plugin | BuiltinPlugin> =
        await flattenPlugins(plugins);

    return flattened.map((plugin: Plugin | BuiltinPlugin): RawPlugin =>
        toRawPlugin({ plugin }),
    );
};

export type { RawPlugin };
export { isAst, toAstInput, toRawPlugin, toRawPlugins };
