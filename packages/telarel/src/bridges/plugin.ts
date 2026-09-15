import type { Program } from "@oxc-project/types";

import type { Options, Plugin, PluginContext } from "#/@types/plugin";

type RawPluginContext = {
    cwd: string;
    file: string;
    code: string;
    metadata: Record<string, unknown>;
};

type RawStageArgs = { file: string; code: string };

type RawTransformArgs = { file: string; astJson: string };

type RawOptionsArgs = {
    cwd: string;
    file: string;
    code: string;
};

type RawOptionsOutput = { cwd: string; file: string; code: string };

type RawTransformOutput = { astJson: string } | null;

type RawPlugin = {
    name: string;
    options?: (options: RawOptionsArgs) => RawOptionsOutput | null;
    pre?: (ctx: RawPluginContext, args: RawStageArgs) => unknown;
    transform?: (
        ctx: RawPluginContext,
        args: RawTransformArgs,
    ) => RawTransformOutput | Promise<RawTransformOutput>;
    post?: (ctx: RawPluginContext, args: RawStageArgs) => unknown;
};

const toRawPlugin = (
    plugin: Plugin,
    metadata: Map<string, unknown>,
): RawPlugin => {
    if (typeof plugin.name !== "string" || plugin.name.length === 0) {
        throw new TypeError("plugin.name must be a non-empty string");
    }

    const name: string = plugin.name;
    const raw: RawPlugin = { name };

    const toContext = (ctx: RawPluginContext): PluginContext => ({
        cwd: ctx.cwd,
        file: ctx.file,
        code: ctx.code,
        metadata,
    });

    const options = plugin.options;

    if (typeof options === "function") {
        raw.options = (rawArgs: RawOptionsArgs): RawOptionsOutput => {
            const next: Options = options({
                cwd: rawArgs.cwd,
                file: rawArgs.file,
                code: rawArgs.code,
            });

            return { cwd: next.cwd, file: next.file, code: next.code };
        };
    }

    const pre = plugin.pre;

    if (typeof pre === "function") {
        raw.pre = async (
            ctx: RawPluginContext,
            args: RawStageArgs,
        ): Promise<void> => {
            await pre(toContext(ctx), args);
        };
    }

    const transform = plugin.transform;

    if (typeof transform === "function") {
        raw.transform = async (
            ctx: RawPluginContext,
            rawArgs: RawTransformArgs,
        ): Promise<RawTransformOutput> => {
            const ast: Program = JSON.parse(rawArgs.astJson) as Program;

            const out: unknown = await transform(toContext(ctx), {
                file: rawArgs.file,
                ast,
            });

            if (out === null || typeof out !== "object") {
                return null;
            }

            const replacement = out as { ast?: Program };

            if (replacement.ast === void 0) {
                throw new TypeError(
                    `Plugin "${name}" transform returned an object without an "ast" property`,
                );
            }

            return { astJson: JSON.stringify(replacement.ast) };
        };
    }

    const post = plugin.post;

    if (typeof post === "function") {
        raw.post = async (
            ctx: RawPluginContext,
            args: RawStageArgs,
        ): Promise<void> => {
            await post(toContext(ctx), args);
        };
    }

    return raw;
};

export type { RawPlugin };
export { toRawPlugin };
