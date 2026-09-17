import type { Program } from "@oxc-project/types";
import type { Format, Partial } from "ts-vista";

// Note: `PluginContext.metadata` is a real `Map<string, unknown>`, created
// once per `compile()` call by this package's wrapper and injected into
// every hook context. The native binding materializes its shared metadata
// as a plain object (napi cannot bridge `Map`); the wrapper ignores that
// object and substitutes the wrapper-owned Map, so all plugins in one
// compilation observe the same `Map` instance.

type PluginContext = {
    /**
     * Current working directory.
     */
    cwd: string;
    /**
     * Original file.
     */
    file: string;
    /**
     * Original code.
     */
    code: string;
    /**
     * Shared cross-plugin store, alive for the whole compilation.
     */
    metadata: Map<string, unknown>;
};

/**
 * Arguments for the `pre` hook.
 */
type PreArgs = {
    /**
     * The file to be compiled.
     */
    file: string;
    /**
     * The code to be compiled.
     */
    code: string;
};

/**
 * Arguments for the `post` hook.
 */
type PostArgs = {
    /**
     * The file that was compiled.
     */
    file: string;
    /**
     * The code that was compiled.
     */
    code: string;
};

/**
 * Arguments for the `transform` hook.
 */
type TransformArgs = {
    /**
     * The file being compiled.
     */
    file: string;
    /**
     * The current AST.
     */
    ast: Program;
};

/**
 * Output of the `transform` hook.
 */
type TransformOutput = { ast: Program };

/**
 * Arguments for the `options` hook.
 */
type Options = {
    /**
     * Current working directory.
     */
    cwd: string;
    /**
     * The file to be compiled.
     */
    file: string;
    /**
     * The code to be compiled.
     */
    code: string;
};

/**
 * Partial options a plugin may return from the `options` hook; omitted fields
 * keep their current values.
 */
type PartialOptions = Format<Partial<Options>>;

/**
 * The plugin.
 */
type Plugin = {
    /**
     * The plugin name.
     */
    name: string;
    /**
     * Update compiler options and return them.
     */
    options?: (options: Options) => PartialOptions | null | void;
    /**
     * Setup.
     */
    pre?: (ctx: PluginContext, args: PreArgs) => void | Promise<void>;
    /**
     * AST-level transform: ast -> ast. Chained. Return { ast } (the
     * possibly-mutated tree) to hand the payload to the next plugin; return
     * nothing to pass through.
     */
    transform?: (
        ctx: PluginContext,
        args: TransformArgs,
    ) => TransformOutput | null | void | Promise<TransformOutput | null | void>;
    /**
     * Cleanup.
     */
    post?: (ctx: PluginContext, args: PostArgs) => void | Promise<void>;
};

export type {
    Options,
    PartialOptions,
    Plugin,
    PluginContext,
    PostArgs,
    PreArgs,
    TransformArgs,
    TransformOutput,
};
