import type {
    CommonPluginContext,
    PluginContext,
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
import type { Options, OptionsArgs } from "#/@types/plugin/options";

/**
 * The order a hook requests relative to the normal registration order.
 */
type PluginOrder = "pre" | "post";

/**
 * A hook in plain function form, or an object carrying an `order`.
 */
type ObjectHook<T> = T | { order?: PluginOrder; handler: T };

/**
 * Hooks are return-based: return the new value to replace it, or `null`/`void`
 * to make no change.
 */
type Plugin = {
    /**
     * The plugin name.
     */
    name: string;
    /**
     * The options hook: return the full options bag (incl. `plugins`) to
     * replace it, or `null`/`void` for no change.
     *
     * Runs before resolution, as a fixpoint: plugins added here get their own
     * `options` hook run too.
     */
    options?: ObjectHook<
        (
            ctx: CommonPluginContext,
            args: OptionsArgs,
        ) => Options | null | void | Promise<Options | null | void>
    >;
    /**
     * The compileStart hook: notify on the start of compilation, with resolved
     * read-only options.
     */
    compileStart?: ObjectHook<
        (ctx: PluginContext, args: CompileStartArgs) => void | Promise<void>
    >;
    /**
     * The prepare hook: run before parsing; return `{ code, map? }` to replace
     * the source, or `null`/`void` for no change.
     */
    prepare?: ObjectHook<
        (
            ctx: PluginContext,
            args: PrepareArgs,
        ) => PrepareResult | null | void | Promise<PrepareResult | null | void>
    >;
    /**
     * The transform hook: chained AST transformation; return `{ ast }` to
     * replace the AST, or `null`/`void` for no change.
     */
    transform?: ObjectHook<
        (
            ctx: PluginContext,
            args: TransformArgs,
        ) =>
            | TransformResult
            | null
            | void
            | Promise<TransformResult | null | void>
    >;
    /**
     * The finalize hook: run after codegen; return `{ code, map? }` to replace
     * the output, or `null`/`void` for no change.
     */
    finalize?: ObjectHook<
        (
            ctx: PluginContext,
            args: FinalizeArgs,
        ) =>
            | FinalizeResult
            | null
            | void
            | Promise<FinalizeResult | null | void>
    >;
    /**
     * The compileEnd hook: notify on the end of compilation.
     *
     * Runs on every path after the options fixpoint settles, including when the
     * compile failed. Not run when the options stage itself fails.
     */
    compileEnd?: ObjectHook<
        (ctx: PluginContext, args: CompileEndArgs) => void | Promise<void>
    >;
};

export type { ObjectHook, Plugin, PluginOrder };
