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
type ObjectHook<T> =
    | T
    | {
          /**
           * The relative order requested for the hook.
           */
          order?: PluginOrder;
          /**
           * The hook implementation.
           */
          handler: T;
      };

/**
 * A named plugin with optional hooks for the pipeline.
 *
 * Hooks are return-based. Return a new value to replace the current one, or
 * `null`/`void` to keep it. The `start`, `prepare`, `finalize`, and `end` hooks
 * are observers. Their return values are ignored.
 */
type Plugin = {
    /**
     * A non-empty string identifying the plugin.
     *
     * Resolved options list the names in registration order, duplicates
     * included.
     */
    name: string;
    /**
     * The options hook. Return the full options bag to replace it, or
     * `null`/`void` for no change. The `code` is never part of the bag.
     *
     * Runs before resolution as a fixpoint. Plugins added here get their own
     * `options` hook run too.
     */
    options?: ObjectHook<
        (
            ctx: CommonPluginContext,
            args: OptionsArgs,
        ) => OptionsArgs | null | void | Promise<OptionsArgs | null | void>
    >;
    /**
     * The start hook. Observes the start of compilation with the resolved
     * read-only options.
     */
    start?: ObjectHook<
        (ctx: PluginContext, args: StartArgs) => void | Promise<void>
    >;
    /**
     * The parse hook. The first non-null result wins. Return `{ ast }` to
     * supply the AST, or `null`/`void` to defer to the next `parse` hook. After
     * every hook defers, the builtin parser runs.
     */
    parse?: ObjectHook<
        (
            ctx: PluginContext,
            args: ParseArgs,
        ) => AstResult | null | void | Promise<AstResult | null | void>
    >;
    /**
     * The prepare hook. Observes the parsed AST. Return values are ignored.
     */
    prepare?: ObjectHook<
        (ctx: PluginContext, args: PrepareArgs) => void | Promise<void>
    >;
    /**
     * The transform hook. The transform hooks run in a chain. Return `{ ast }`
     * to replace the AST, or `null`/`void` for no change.
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
     * The finalize hook. Observes the final AST. Return values are ignored.
     */
    finalize?: ObjectHook<
        (ctx: PluginContext, args: FinalizeArgs) => void | Promise<void>
    >;
    /**
     * The generate hook. The first non-null result wins. Return a result object
     * with `code` and an optional `map` to supply the output, or `null`/`void`
     * to defer to the next `generate` hook. After every hook defers, the
     * builtin generator runs.
     */
    generate?: ObjectHook<
        (
            ctx: PluginContext,
            args: GenerateArgs,
        ) => CodeResult | null | void | Promise<CodeResult | null | void>
    >;
    /**
     * The end hook. Observes the end of compilation.
     *
     * Runs on every path after the options fixpoint settles, even when
     * compilation fails. Does not run when the options stage itself fails.
     */
    end?: ObjectHook<
        (ctx: PluginContext, args: EndArgs) => void | Promise<void>
    >;
};

export type { ObjectHook, Plugin, PluginOrder };
