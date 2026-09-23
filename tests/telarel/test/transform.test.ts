import type {
    BuiltinPlugin,
    CompileResult,
    Plugin,
    PluginContext,
    TransformArgs,
} from "telarel";

import { compile } from "telarel";
import { transform } from "telarel/plugins/transform";
import { describe, expect, it } from "vitest";

describe("transform", (): void => {
    it("compiles TypeScript to JavaScript with the builtin transform", async (): Promise<void> => {
        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a: number = 1;",
            plugins: [transform()],
        });

        expect(result.code).not.toContain(": number");
        expect(result.code).toContain("const a");
        expect(result.map.version).toBe(3);
    });

    it("lowers async await with an es2015 target", async (): Promise<void> => {
        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.js",
            code: "async function run() { await work(); }",
            plugins: [transform({ targets: ["es2015"] })],
        });

        expect(result.code).toContain("_asyncToGenerator");
        expect(result.code).toContain("function run()");
    });

    it("accepts a one-key engine object target", async (): Promise<void> => {
        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.js",
            code: "const value = maybe?.prop;",
            plugins: [transform({ targets: [{ chrome: 58 }] })],
        });

        expect(result.code).not.toContain("?.");
    });

    it("transforms jsx with the automatic runtime", async (): Promise<void> => {
        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.jsx",
            code: "const node = <Button>hi</Button>;",
            plugins: [transform({ jsx: { runtime: "automatic" } })],
        });

        expect(result.code).not.toContain("<Button>");
        expect(result.code).toContain("jsx");
    });

    it("applies typed oxc escape-hatch options", async (): Promise<void> => {
        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.tsx",
            code: "const node = <Button>hi</Button>;",
            plugins: [
                transform({
                    oxc: { jsx: { runtime: "classic", pragma: "h" } },
                }),
            ],
        });

        expect(result.code).toContain("h(");
    });

    it("applies oxc helper loader options", async (): Promise<void> => {
        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.js",
            code: "async function run() { await work(); }",
            plugins: [
                transform({
                    targets: ["es2015"],
                    oxc: { helperLoader: { mode: "external" } },
                }),
            ],
        });

        expect(result.code).toContain("babelHelpers.");
    });

    it("treats an unbranded plugin named builtin:* as a js plugin", async (): Promise<void> => {
        const plugin: unknown = { name: "builtin:custom" };

        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [plugin as BuiltinPlugin],
        });

        expect(result.code).toBe("const a = 1;");
    });

    it("runs alongside plain js plugins in one array", async (): Promise<void> => {
        const seen: Array<unknown> = [];

        const jsPlugin: Plugin = {
            name: "observer",
            transform: (ctx: PluginContext): void => {
                seen.push(ctx.metadata.get("marker"));
            },
        };

        const writer: Plugin = {
            name: "writer",
            pre: (ctx: PluginContext): void => {
                ctx.metadata.set("marker", "from-writer");
            },
        };

        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a: number = 1;",
            plugins: [writer, transform(), jsPlugin],
        });

        expect(result.code).not.toContain(": number");
        expect(seen).toEqual(["from-writer"]);
    });

    it("gives a plain js transform hook the post-builtin ast", async (): Promise<void> => {
        const seen: Array<unknown> = [];

        const jsPlugin: Plugin = {
            name: "observer",
            transform: (ctx: PluginContext, args: TransformArgs): void => {
                const body: Array<{ type: string }> = args.ast.body as Array<{
                    type: string;
                }>;

                ctx.metadata.set("firstNode", body[0]?.type ?? "missing");
            },
            post: (ctx: PluginContext): void => {
                seen.push(ctx.metadata.get("firstNode"));
            },
        };

        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a: number = 1;",
            plugins: [transform(), jsPlugin],
        });

        expect(result.code).not.toContain(": number");
        expect(seen).toEqual(["VariableDeclaration"]);
    });

    it("accepts a hand-written marker with __builtin true", async (): Promise<void> => {
        const marker: unknown = {
            __builtin: true,
            name: "builtin:transform",
            options: { targets: ["nonsense"] },
        };

        const build = async (): Promise<CompileResult> =>
            await compile({
                cwd: "/repo",
                file: "index.ts",
                code: "const a = 1;",
                plugins: [marker as BuiltinPlugin],
            });

        await expect(build()).rejects.toThrow("nonsense");
    });

    it("rejects an unknown builtin name", async (): Promise<void> => {
        const marker: unknown = {
            __builtin: true,
            name: "builtin:nope",
        };

        const build = async (): Promise<CompileResult> =>
            await compile({
                cwd: "/repo",
                file: "index.ts",
                code: "const a = 1;",
                plugins: [marker as BuiltinPlugin],
            });

        const promise: Promise<CompileResult> = build();

        await expect(promise).rejects.toThrow(/builtin/);
        await expect(promise).rejects.toThrow("builtin:nope");
    });

    it("treats a js plugin named builtin:* with hooks as a js plugin", async (): Promise<void> => {
        const seen: Array<unknown> = [];

        const plugin: Plugin = {
            name: "builtin:custom",
            pre: (ctx: PluginContext): void => {
                ctx.metadata.set("marker", "from-builtin-custom");
            },
            transform: (ctx: PluginContext): void => {
                seen.push(ctx.metadata.get("marker"));
            },
        };

        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [plugin],
        });

        expect(result.code).toBe("const a = 1;");
        expect(seen).toEqual(["from-builtin-custom"]);
    });
});
