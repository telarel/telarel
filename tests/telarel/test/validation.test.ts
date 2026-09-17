import type { CompileResult, Options, Plugin, PluginContext } from "telarel";

import { compile } from "telarel";
import { describe, expect, it } from "vitest";

describe("validation", (): void => {
    it("rejects on a parse error with file context", async (): Promise<void> => {
        // A plugin declaring `transform` forces the parse path; without one
        // the skip path passes invalid code through verbatim. Pinned
        // observed message: the native parser renders a code-frame style
        // diagnostic including the file name and caret position.
        await expect(
            compile({
                cwd: "/repo",
                file: "index.ts",
                code: "const = ;",
                plugins: [
                    {
                        name: "force-parse",
                        transform: (): void => void 0,
                    },
                ],
            }),
        ).rejects.toThrow("index.ts");

        await expect(
            compile({
                cwd: "/repo",
                file: "index.ts",
                code: "const = ;",
                plugins: [
                    {
                        name: "force-parse",
                        transform: (): void => void 0,
                    },
                ],
            }),
        ).rejects.toThrow(/expected|Unexpected token/i);
    });

    it("passes invalid code through verbatim when no transform plugin exists", async (): Promise<void> => {
        // Skip path: without a `transform` hook the source is never parsed,
        // so syntactically invalid code compiles to itself.
        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const = ;",
        });

        expect(result.code).toBe("const = ;");
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

    it("leaves options unchanged when an options hook returns null", async (): Promise<void> => {
        // Intentional behavior change: a `null` return is a pass-through.
        // The wrapper maps it to Rust `None`, so the compile options are
        // carried over verbatim.
        const plugin: unknown = {
            name: "null-options",
            options: (): null => null,
        };

        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [plugin as Plugin],
        });

        expect(result.code).toBe("const a = 1;");
    });

    it("leaves options unchanged when an options hook returns undefined", async (): Promise<void> => {
        // Companion to the `null` case above; `void` returns behave the
        // same as explicit `null`.
        const plugin: unknown = {
            name: "void-options",
            options: (): void => void 0,
        };

        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [plugin as Plugin],
        });

        expect(result.code).toBe("const a = 1;");
    });

    it("merges a partial options hook return with the current options", async (): Promise<void> => {
        // Intentional behavior change: partial returns no longer reject.
        // Omitted fields keep their current values; here only `code` is
        // replaced while `cwd` and `file` carry over into the transform.
        const plugin: unknown = {
            name: "partial-options",
            options: (): unknown => ({ code: "const b = 2;" }),
            transform: (ctx: PluginContext): { ast: unknown } | null => {
                expect(ctx.cwd).toBe("/repo");
                expect(ctx.file).toBe("index.ts");
                expect(ctx.code).toBe("const b = 2;");

                return null;
            },
        };

        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [plugin as Plugin],
        });

        expect(result.code).toBe("const b = 2;");
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
