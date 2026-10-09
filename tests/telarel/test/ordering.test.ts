import type {
    CommonPluginContext,
    CompileResult,
    OptionsArgs,
    Plugin,
    PluginContext,
    TransformArgs,
} from "telarel";

import { compile } from "telarel";
import { describe, expect, it } from "vitest";

const isEnabled = (): boolean => false;

describe("ordering", (): void => {
    it("orders each hook independently across plugins", async (): Promise<void> => {
        const seenPrepare: Array<string> = [];
        const seenTransform: Array<string> = [];

        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                {
                    name: "a",
                    prepare: (): void => {
                        seenPrepare.push("a");
                    },
                    transform: (): void => {
                        seenTransform.push("a");
                    },
                },
                {
                    name: "b",
                    prepare: {
                        order: "pre",
                        handler: (): void => {
                            seenPrepare.push("b");
                        },
                    },
                    transform: {
                        order: "post",
                        handler: (): void => {
                            seenTransform.push("b");
                        },
                    },
                },
                {
                    name: "c",
                    prepare: {
                        order: "post",
                        handler: (): void => {
                            seenPrepare.push("c");
                        },
                    },
                    transform: {
                        order: "pre",
                        handler: (): void => {
                            seenTransform.push("c");
                        },
                    },
                },
            ],
        });

        expect(seenPrepare).toEqual(["b", "a", "c"]);
        expect(seenTransform).toEqual(["c", "a", "b"]);
        expect(result.code).toBe("const a = 1;");
    });

    it("orders a single plugin's hooks independently", async (): Promise<void> => {
        const seenPrepare: Array<string> = [];
        const seenFinalize: Array<string> = [];

        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                {
                    name: "meta",
                    prepare: {
                        order: "pre",
                        handler: (): void => {
                            seenPrepare.push("meta");
                        },
                    },
                    finalize: {
                        order: "post",
                        handler: (): void => {
                            seenFinalize.push("meta");
                        },
                    },
                },
                {
                    name: "plain",
                    prepare: (): void => {
                        seenPrepare.push("plain");
                    },
                    finalize: (): void => {
                        seenFinalize.push("plain");
                    },
                },
            ],
        });

        expect(seenPrepare).toEqual(["meta", "plain"]);
        expect(seenFinalize).toEqual(["plain", "meta"]);
        expect(result.code).toBe("const a = 1;");
    });

    it("keeps plain hooks in the normal bucket in registration order", async (): Promise<void> => {
        const seen: Array<string> = [];

        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                {
                    name: "plain-a",
                    prepare: (): void => {
                        seen.push("plain-a");
                    },
                },
                {
                    name: "pre",
                    prepare: {
                        order: "pre",
                        handler: (): void => {
                            seen.push("pre");
                        },
                    },
                },
                {
                    name: "plain-b",
                    prepare: (): void => {
                        seen.push("plain-b");
                    },
                },
                {
                    name: "post",
                    prepare: {
                        order: "post",
                        handler: (): void => {
                            seen.push("post");
                        },
                    },
                },
            ],
        });

        expect(seen).toEqual(["pre", "plain-a", "plain-b", "post"]);
        expect(result.code).toBe("const a = 1;");
    });

    it("rejects an unknown hook order", async (): Promise<void> => {
        const sideways: unknown = {
            name: "sideways",
            transform: {
                order: "sideways",
                handler: (_ctx: PluginContext, _args: TransformArgs): void =>
                    void 0,
            },
        };

        const build = async (): Promise<CompileResult> =>
            await compile({
                cwd: "/repo",
                file: "index.ts",
                code: "const a = 1;",
                plugins: [sideways as Plugin],
            });

        await expect(build()).rejects.toThrow("unknown plugin order");
        await expect(build()).rejects.toThrow("sideways");
    });

    it("composes spread plugins in order", async (): Promise<void> => {
        const seen: Array<string> = [];

        const prePlugin = (): Plugin => ({
            name: "pre",
            prepare: {
                order: "pre",
                handler: (): void => {
                    seen.push("pre");
                },
            },
        });

        const shared = (): Array<Plugin> => [
            {
                name: "shared-a",
                prepare: (): void => {
                    seen.push("shared-a");
                },
            },
            {
                name: "shared-b",
                prepare: (): void => {
                    seen.push("shared-b");
                },
            },
        ];

        const postPlugin = (): Plugin => ({
            name: "post",
            prepare: {
                order: "post",
                handler: (): void => {
                    seen.push("post");
                },
            },
        });

        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [prePlugin(), ...shared(), postPlugin()],
        });

        expect(seen).toEqual(["pre", "shared-a", "shared-b", "post"]);
        expect(result.code).toBe("const a = 1;");
    });

    it("flattens nested plugin arrays in order", async (): Promise<void> => {
        const seen: Array<string> = [];

        const collect = (name: string): Plugin => ({
            name,
            prepare: (): void => {
                seen.push(name);
            },
        });

        const myPlugins = (): Array<Plugin> => [
            collect("my-a"),
            collect("my-b"),
        ];

        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [myPlugins(), collect("my-c"), [collect("my-d")]],
        });

        expect(seen).toEqual(["my-a", "my-b", "my-c", "my-d"]);
        expect(result.code).toBe("const a = 1;");
    });

    it("skips falsy conditional plugin entries", async (): Promise<void> => {
        const seen: Array<string> = [];

        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                false,
                null,
                void 0,
                isEnabled() && {
                    name: "skipped",
                    prepare: (): void => {
                        seen.push("skipped");
                    },
                },
                {
                    name: "kept",
                    prepare: (): void => {
                        seen.push("kept");
                    },
                },
            ],
        });

        expect(seen).toEqual(["kept"]);
        expect(result.code).toBe("const a = 1;");
    });

    it("awaits promise plugin entries", async (): Promise<void> => {
        const seen: Array<string> = [];

        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                Promise.resolve({
                    name: "promised",
                    prepare: (): void => {
                        seen.push("promised");
                    },
                }),
                Promise.resolve([
                    {
                        name: "promised-nested",
                        prepare: (): void => {
                            seen.push("promised-nested");
                        },
                    },
                ]),
            ],
        });

        expect(seen).toEqual(["promised", "promised-nested"]);
        expect(result.code).toBe("const a = 1;");
    });

    it("rejects a bare function plugin entry", async (): Promise<void> => {
        const factory: Plugin = (() => void 0) as unknown as Plugin;

        const build = async (): Promise<CompileResult> =>
            await compile({
                cwd: "/repo",
                file: "index.ts",
                code: "const a = 1;",
                plugins: [factory],
            });

        await expect(build()).rejects.toThrow(TypeError);
        await expect(build()).rejects.toThrow(
            "pass a plugin object, not a factory",
        );
    });

    it("runs pre options hooks before normal and applies last-wins", async (): Promise<void> => {
        const seen: Array<string> = [];

        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                {
                    name: "normal",
                    options: (
                        _ctx: CommonPluginContext,
                        args: OptionsArgs,
                    ): OptionsArgs => {
                        seen.push("normal");

                        return {
                            options: {
                                ...args.options,
                                code: "const normal = 1;",
                            },
                        };
                    },
                },
                {
                    name: "pre",
                    options: {
                        order: "pre",
                        handler: (
                            _ctx: CommonPluginContext,
                            args: OptionsArgs,
                        ): OptionsArgs => {
                            seen.push("pre");

                            return {
                                options: {
                                    ...args.options,
                                    code: "const pre = 1;",
                                },
                            };
                        },
                    },
                },
            ],
        });

        expect(seen).toEqual(["pre", "normal"]);
        expect(result.code).toBe("const normal = 1;");
    });

    it("orders compileStart hooks across plugins", async (): Promise<void> => {
        const seen: Array<string> = [];

        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                {
                    name: "normal",
                    compileStart: (): void => {
                        seen.push("normal");
                    },
                },
                {
                    name: "pre",
                    compileStart: {
                        order: "pre",
                        handler: (): void => {
                            seen.push("pre");
                        },
                    },
                },
                {
                    name: "post",
                    compileStart: {
                        order: "post",
                        handler: (): void => {
                            seen.push("post");
                        },
                    },
                },
            ],
        });

        expect(seen).toEqual(["pre", "normal", "post"]);
        expect(result.code).toBe("const a = 1;");
    });

    it("orders compileEnd hooks across plugins", async (): Promise<void> => {
        const seen: Array<string> = [];

        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                {
                    name: "normal",
                    compileEnd: (): void => {
                        seen.push("normal");
                    },
                },
                {
                    name: "pre",
                    compileEnd: {
                        order: "pre",
                        handler: (): void => {
                            seen.push("pre");
                        },
                    },
                },
                {
                    name: "post",
                    compileEnd: {
                        order: "post",
                        handler: (): void => {
                            seen.push("post");
                        },
                    },
                },
            ],
        });

        expect(seen).toEqual(["pre", "normal", "post"]);
        expect(result.code).toBe("const a = 1;");
    });

    it("orders finalize hooks across plugins", async (): Promise<void> => {
        const seen: Array<string> = [];

        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                {
                    name: "normal",
                    finalize: (): void => {
                        seen.push("normal");
                    },
                },
                {
                    name: "pre",
                    finalize: {
                        order: "pre",
                        handler: (): void => {
                            seen.push("pre");
                        },
                    },
                },
                {
                    name: "post",
                    finalize: {
                        order: "post",
                        handler: (): void => {
                            seen.push("post");
                        },
                    },
                },
            ],
        });

        expect(seen).toEqual(["pre", "normal", "post"]);
        expect(result.code).toBe("const a = 1;");
    });

    it("treats an object hook without order as normal", async (): Promise<void> => {
        const seen: Array<string> = [];

        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                {
                    name: "normal-object",
                    prepare: {
                        handler: (): void => {
                            seen.push("normal-object");
                        },
                    },
                },
                {
                    name: "pre",
                    prepare: {
                        order: "pre",
                        handler: (): void => {
                            seen.push("pre");
                        },
                    },
                },
                {
                    name: "post",
                    prepare: {
                        order: "post",
                        handler: (): void => {
                            seen.push("post");
                        },
                    },
                },
            ],
        });

        expect(seen).toEqual(["pre", "normal-object", "post"]);
        expect(result.code).toBe("const a = 1;");
    });
});
