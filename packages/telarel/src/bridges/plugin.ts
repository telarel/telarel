import type { Program } from "@oxc-project/types";

import type {
    Options,
    Plugin,
    PluginContext,
    TransformArgs,
} from "#/@types/plugin";

type RawPluginContext = {
    cwd: string;
    file: string;
    code: string;
};

type RawStageArgs = { file: string; code: string };

type RawTransformArgs = { file: string; astJson: string };

type RawOptionsArgs = {
    cwd: string;
    file: string;
    code: string;
};

type RawOptionsOutput = {
    cwd: string;
    file: string;
    code: string;
};

type RawTransformOutput = { astJson: string } | null;

type RawPlugin = {
    name: string;
    options?: (
        options: RawOptionsArgs,
    ) => RawOptionsOutput | Promise<RawOptionsOutput>;
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
        raw.options = async (
            rawArgs: RawOptionsArgs,
        ): Promise<RawOptionsOutput> => {
            const current: Options = {
                cwd: rawArgs.cwd,
                file: rawArgs.file,
                code: rawArgs.code,
            };

            await options(current);

            const output: RawOptionsOutput = {
                cwd: rawArgs.cwd,
                file: rawArgs.file,
                code: rawArgs.code,
            };

            if (typeof current.cwd === "string") {
                output.cwd = current.cwd;
            }

            if (typeof current.file === "string") {
                output.file = current.file;
            }

            if (typeof current.code === "string") {
                output.code = current.code;
            }

            return output;
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
        ): Promise<RawTransformOutput | null> => {
            const ast: Program = JSON.parse(rawArgs.astJson) as Program;

            const args: TransformArgs = { file: rawArgs.file, ast };

            await transform(toContext(ctx), args);

            const next: string = JSON.stringify(args.ast);

            if (next === rawArgs.astJson) return null;

            return { astJson: next };
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
