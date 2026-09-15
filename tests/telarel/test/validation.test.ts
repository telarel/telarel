import type { CompileResult, Options, Plugin, PluginContext } from "telarel";

import { compile } from "telarel";
import { describe, expect, it } from "vitest";

describe("validation", (): void => {
    it("rejects on a parse error with file context", async (): Promise<void> => {
        // Pinned observed message: the native parser renders a code-frame
        // style diagnostic including the file name and caret position.
        await expect(
            compile({
                cwd: "/repo",
                file: "index.ts",
                code: "const = ;",
            }),
        ).rejects.toThrow("index.ts");
        await expect(
            compile({
                cwd: "/repo",
                file: "index.ts",
                code: "const = ;",
            }),
        ).rejects.toThrow(/expected|Unexpected token/i);
    });

    it("rejects on missing required compile options", async (): Promise<void> => {
        // Pinned observed behavior: the napi bridge validates the options
        // record before the pipeline runs and rejects with a serde-style
        // `Missing field` error naming the first absent field, regardless
        // of field order.
        const noCwd: unknown = {
            file: "index.ts",
            code: "const a = 1;",
        };
        const noCode: unknown = {
            cwd: "/repo",
            file: "index.ts",
        };

        await expect(compile(noCwd as Options)).rejects.toThrow(
            "Missing field `cwd`",
        );
        await expect(compile(noCode as Options)).rejects.toThrow(
            "Missing field `code`",
        );
    });

    it("rejects when an options hook returns null", async (): Promise<void> => {
        // Pinned observed message: "options hook: PendingException, Cannot
        // read properties of null (reading 'cwd')". The wrapper at
        // packages/telarel/src/bridges/plugin.ts dereferences `next.cwd`
        // without a null guard, so the hook result flows into the TSFN
        // bridge as a thrown TypeError surfaced as a PendingException.
        const plugin: unknown = {
            name: "null-options",
            options: (): null => null,
        };

        const build = async (): Promise<CompileResult> =>
            await compile({
                cwd: "/repo",
                file: "index.ts",
                code: "const a = 1;",
                plugins: [plugin as Plugin],
            });

        await expect(build()).rejects.toThrow(
            "Cannot read properties of null (reading 'cwd')",
        );
    });

    it("rejects when an options hook returns a partial object", async (): Promise<void> => {
        // Pinned observed message: "options hook: InvalidArg, Value is none
        // of these types `Promise`, `JsOptionsOutput`, ". The Rust side
        // receives the partial object via TSFN and fails its downcast before
        // any property access, producing an InvalidArg conversion error
        // (the message ends with a trailing comma and space).
        const plugin: unknown = {
            name: "partial-options",
            options: (): unknown => ({ cwd: "/x", file: "a.ts" }),
        };

        const build = async (): Promise<CompileResult> =>
            await compile({
                cwd: "/repo",
                file: "index.ts",
                code: "const a = 1;",
                plugins: [plugin as Plugin],
            });

        await expect(build()).rejects.toThrow(
            "Value is none of these types `Promise`, `JsOptionsOutput`",
        );
    });

    it("allows duplicate plugin names in registration order", async (): Promise<void> => {
        const seen: Array<string> = [];

        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                {
                    name: "dup",
                    pre: (ctx: PluginContext): void => {
                        seen.push("dup.first");
                        ctx.metadata.set("marker", "first");
                    },
                },
                {
                    name: "dup",
                    pre: (ctx: PluginContext): void => {
                        seen.push("dup.second");
                        seen.push(String(ctx.metadata.get("marker")));
                    },
                },
            ],
        });

        expect(result.code).toBe("const a = 1;");
        expect(seen).toEqual(["dup.first", "dup.second", "first"]);
    });
});
