import type { AstResult, CodeResult } from "#/@types/ast";
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
import type { OptionsArgs } from "#/@types/plugin/options";

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
 * to make no change. The `start`, `prepare`, `finalize`, and `end` hooks are
 * observers: return values are ignored.
 */
type Plugin = {
    /**
     * The plugin name.
     */
    name: string;
    /**
     * The options hook: return the full options args to replace it, or
     * `null`/`void` for no change. The `code` is never part of the bag.
     *
     * Runs before resolution, as a fixpoint: plugins added here get their own
     * `options` hook run too.
     */
    options?: ObjectHook<
        (
            ctx: CommonPluginContext,
            args: OptionsArgs,
        ) => OptionsArgs | null | void | Promise<OptionsArgs | null | void>
    >;
    /**
     * The start hook: notify on the start of compilation, with resolved
     * read-only options.
     */
    start?: ObjectHook<
        (ctx: PluginContext, args: StartArgs) => void | Promise<void>
    >;
    /**
     * The parse hook: first-win. Return `{ ast }` to supply the AST, or
     * `null`/`void` to defer to the next `parse` hook and, after all defer, the
     * builtin parser.
     */
    parse?: ObjectHook<
        (
            ctx: PluginContext,
            args: ParseArgs,
        ) => AstResult | null | void | Promise<AstResult | null | void>
    >;
    /**
     * The prepare hook: observer on the parsed AST; return values are ignored.
     */
    prepare?: ObjectHook<
        (ctx: PluginContext, args: PrepareArgs) => void | Promise<void>
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
     * The finalize hook: observer on the final AST; return values are ignored.
     */
    finalize?: ObjectHook<
        (ctx: PluginContext, args: FinalizeArgs) => void | Promise<void>
    >;
    /**
     * The generate hook: first-win. Return `{ code, map? }` to supply the
     * output, or `null`/`void` to defer to the next `generate` hook and, after
     * all defer, the builtin generator.
     */
    generate?: ObjectHook<
        (
            ctx: PluginContext,
            args: GenerateArgs,
        ) => CodeResult | null | void | Promise<CodeResult | null | void>
    >;
    /**
     * The end hook: notify on the end of compilation.
     *
     * Runs on every path after the options fixpoint settles, including when the
     * compile failed. Not run when the options stage itself fails.
     */
    end?: ObjectHook<
        (ctx: PluginContext, args: EndArgs) => void | Promise<void>
    >;
};

export type { ObjectHook, Plugin, PluginOrder };
