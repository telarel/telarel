import type { Program } from "@oxc-project/types";
import type { Format, Partial } from "ts-vista";

// Note: `PluginContext.metadata` is a real `Map<string, unknown>`, created
// once per `compile()` call by this package's wrapper and injected into
// every hook context, so all plugins in one compilation observe the same
// `Map` instance.

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
     * The current AST; mutate it in place.
     */
    ast: Program;
};

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
     * The options hook: update compiler options and return them.
     */
    options?: (
        options: Options,
    ) => PartialOptions | null | void | Promise<PartialOptions | null | void>;
    /**
     * The pre hook: run before compilation.
     */
    pre?: (ctx: PluginContext, args: PreArgs) => void | Promise<void>;
    /**
     * The transform hook: chained AST-level transformation.
     */
    transform?: (
        ctx: PluginContext,
        args: TransformArgs,
    ) => void | Promise<void>;
    /**
     * The post hook: run after compilation.
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
};
