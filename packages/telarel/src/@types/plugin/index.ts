import type {
    CommonPluginContext,
    PluginContext,
} from "#/@types/plugin/context";
import type { CompileEndArgs } from "#/@types/plugin/hooks/compile-end";
import type { CompileStartArgs } from "#/@types/plugin/hooks/compile-start";
import type { PostArgs, PostResult } from "#/@types/plugin/hooks/post";
import type { PreArgs, PreResult } from "#/@types/plugin/hooks/pre";
import type {
    TransformArgs,
    TransformResult,
} from "#/@types/plugin/hooks/transform";
import type { Options, OptionsArgs } from "#/@types/plugin/options";

/**
 * Hooks are return-based: return the new value to replace it, or `null`/`void`
 * to make no change.
 */
type Plugin = {
    name: string;
    /**
     * The options hook: return the full options bag (incl. `plugins`) to
     * replace it, or `null`/`void` for no change.
     *
     * Runs before resolution, as a fixpoint: plugins added here get their own
     * `options` hook run too.
     */
    options?: (
        ctx: CommonPluginContext,
        args: OptionsArgs,
    ) => Options | null | void | Promise<Options | null | void>;
    /**
     * The compileStart hook: notify on the start of compilation, with resolved
     * read-only options.
     */
    compileStart?: (
        ctx: PluginContext,
        args: CompileStartArgs,
    ) => void | Promise<void>;
    /**
     * The pre hook: run before parsing; return `{ code }` to replace the
     * source, or `null`/`void` for no change.
     */
    pre?: (
        ctx: PluginContext,
        args: PreArgs,
    ) => PreResult | null | void | Promise<PreResult | null | void>;
    /**
     * The transform hook: chained AST transformation; return `{ ast }` to
     * replace the AST, or `null`/`void` for no change.
     */
    transform?: (
        ctx: PluginContext,
        args: TransformArgs,
    ) => TransformResult | null | void | Promise<TransformResult | null | void>;
    /**
     * The post hook: run after codegen; return `{ code, map? }` to replace the
     * output, or `null`/`void` for no change.
     */
    post?: (
        ctx: PluginContext,
        args: PostArgs,
    ) => PostResult | null | void | Promise<PostResult | null | void>;
    /**
     * The compileEnd hook: notify on the end of compilation.
     *
     * Runs on every path after the options fixpoint settles, including when the
     * compile failed. Not run when the options stage itself fails.
     */
    compileEnd?: (
        ctx: PluginContext,
        args: CompileEndArgs,
    ) => void | Promise<void>;
};

export type { Plugin };
