import type {
    CommonPluginContext,
    CompileOptions,
    CompileResult,
    OptionsArgs,
    Plugin,
    PluginContext,
} from "telarel";

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

    it("rejects on a missing required compile option", async (): Promise<void> => {
        // Pinned observed behavior: the napi bridge validates the options
        // record before the pipeline runs and rejects with a serde-style
        // `Missing field` error naming the absent field. `cwd` is optional
        // and defaults to the process working directory, so `code` is the
        // remaining required field without a default.
        const noCode: unknown = {
            cwd: "/repo",
            file: "index.ts",
        };

        await expect(compile(noCode as CompileOptions)).rejects.toThrow(
            "Missing field `code`",
        );
    });

    it("leaves options unchanged when an options hook returns null", async (): Promise<void> => {
        // `null` = no change: the returned-bag fold keeps the current bag;
        // the compile options are carried over unchanged.
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
        // Companion to the `null` case above; `void` = no change either way
        // and the options bag comes back unchanged.
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

    it("applies a returned options bag", async (): Promise<void> => {
        // Return-based contract: the hook returns the replacement args; the
        // transform observes the new `code` with the current `cwd` and
        // `file` carried over by the spread.
        let stage: string = "__NONE__";

        const plugin: unknown = {
            name: "return-options",
            options: (
                _ctx: CommonPluginContext,
                args: OptionsArgs,
            ): OptionsArgs => {
                stage = "options";

                return {
                    options: {
                        ...args.options,
                        code: "const b = 2;",
                    },
                };
            },
            transform: (ctx: PluginContext): void => {
                expect(stage).toBe("options");
                expect(ctx.cwd).toBe("/repo");
                expect(ctx.module.file).toBe("index.ts");
                expect(ctx.module.code).toBe("const b = 2;");
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

    it("applies a returned bag that omits plugins as full-bag replace", async (): Promise<void> => {
        // Full-bag semantics: a returned bag replacing the whole bag, so a
        // bag that carries only `code` drops the plugin list (default: the
        // EMPTY list) and every omitted scalar falls back to its default.
        // This compile's only plugin replaces the bag without `plugins`,
        // so nobody survives to carry a `code` rewrite: the compile itself
        // uses the returned bag verbatim.
        const plugin: unknown = {
            name: "returned-bag",
            options: (): OptionsArgs => ({
                options: {
                    code: "const replaced = 2;",
                },
            }),
        };

        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [plugin as Plugin],
        });

        expect(result.code).toBe("const replaced = 2;");
    });

    it("allows duplicate plugin names in registration order", async (): Promise<void> => {
        const seen: Array<string> = [];
        let marker: unknown = "__NO_MARKER__";

        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                {
                    name: "dup",
                    prepare: (): void => {
                        seen.push("dup.first");
                        marker = "first";
                    },
                },
                {
                    name: "dup",
                    prepare: (): void => {
                        seen.push("dup.second");
                        seen.push(String(marker));
                    },
                },
            ],
        });

        expect(result.code).toBe("const a = 1;");
        expect(seen).toEqual(["dup.first", "dup.second", "first"]);
    });
});
