import type {
    CommonPluginContext,
    CompileEndArgs,
    CompileResult,
    CompileStartArgs,
    Options,
    OptionsArgs,
    Plugin,
    PluginContext,
    FinalizeArgs,
    FinalizeResult,
    PrepareArgs,
    PrepareResult,
    ResolvedOptions,
    SourceMap,
    TransformArgs,
    TransformResult,
} from "telarel";

import type { ProgramFixture } from "#/functions/ast";

import { compile } from "telarel";
import { walk } from "telarel/walker";
import { describe, expect, it } from "vitest";

import { renameRootIdentifier } from "#/functions/ast";

type CallFixture = {
    body: Array<{
        expression: {
            callee: { object: { type: string } };
        };
    }>;
};

type InitFixture = {
    body: Array<{
        declarations: Array<{
            init: { type: string; value: string };
        }>;
    }>;
};

describe("compile", (): void => {
    it("returns original code with no plugins", async (): Promise<void> => {
        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
        });

        expect(result.code).toBe("const a = 1;");
        expect(result.map.version).toBe(3);
        expect(result.map.mappings.length).toBeGreaterThan(0);
    });

    it("treats a pass-through transform hook as no change", async (): Promise<void> => {
        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                {
                    name: "pass-through",
                    transform: (): void => void 0,
                },
            ],
        });

        expect(result.code).toBe("const a = 1;");
        expect(result.map.version).toBe(3);
        expect(result.map.mappings.length).toBeGreaterThan(0);
    });

    it("runs a visitor from oxc-parser inside transform", async (): Promise<void> => {
        const seen: Array<string> = [];

        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "console.log(1);",
            plugins: [
                {
                    name: "spy",
                    transform: (
                        _ctx: PluginContext,
                        args: TransformArgs,
                    ): void => {
                        // Manual structural traversal: cast `args.ast` to a
                        // narrow type and walk it in place.
                        const program: CallFixture =
                            args.ast as unknown as CallFixture;
                        const statement: CallFixture["body"][number] | void =
                            program.body[0];
                        if (!statement) {
                            throw new Error("expected a top-level statement");
                        }
                        seen.push(statement.expression.callee.object.type);
                    },
                },
            ],
        });

        expect(seen).toEqual(["Identifier"]);
        expect(result.code).toContain("console.log(1)");
        expect(result.map.version).toBe(3);
        expect(result.map.mappings.length).toBeGreaterThan(0);
    });

    it("shares state across plugins and hooks", async (): Promise<void> => {
        const seen: Array<unknown> = [];

        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                {
                    name: "writer",
                    compileStart: (ctx: PluginContext): void => {
                        ctx.state.set("marker", "from-compile-start");
                    },
                },
                {
                    name: "reader",
                    prepare: (ctx: PluginContext): void => {
                        seen.push(ctx.state.get("marker"));
                    },
                },
                {
                    name: "verifier",
                    finalize: (ctx: PluginContext): void => {
                        seen.push(ctx.state.get("marker"));
                    },
                },
            ],
        });

        expect(result.code).toBe("const a = 1;");
        expect(seen).toEqual(["from-compile-start", "from-compile-start"]);
    });

    it("injects the same state Map instance into every hook", async (): Promise<void> => {
        const maps: Array<unknown> = [];

        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                {
                    name: "collect-options",
                    options: (ctx: CommonPluginContext): void => {
                        maps.push(ctx.state);
                    },
                },
                {
                    name: "collect-rest",
                    prepare: (ctx: PluginContext): void => {
                        maps.push(ctx.state);
                    },
                    transform: (ctx: PluginContext): void => {
                        maps.push(ctx.state);
                    },
                    finalize: (ctx: PluginContext): void => {
                        maps.push(ctx.state);
                    },
                },
            ],
        });

        expect(result.code).toBe("const a = 1;");
        expect(maps).toHaveLength(4);
        for (const map of maps) {
            expect(map).toBe(maps[0]);
        }
        expect(new Set(maps).size).toBe(1);
    });

    it("chains options hooks last-wins", async (): Promise<void> => {
        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                {
                    name: "rewrite-b",
                    options: (
                        _ctx: CommonPluginContext,
                        args: OptionsArgs,
                    ): Options | null | void => ({
                        ...args.options,
                        code: "const b = 2;",
                    }),
                },
                {
                    name: "rewrite-c",
                    options: (
                        _ctx: CommonPluginContext,
                        args: OptionsArgs,
                    ): Options | null | void => ({
                        ...args.options,
                        code: "const c = 3;",
                    }),
                },
            ],
        });

        expect(result.code).toContain("const c = 3");
        expect(result.code).not.toContain("const b = 2");
    });

    it("keeps the bag unchanged when an options hook returns null", async (): Promise<void> => {
        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                {
                    name: "rewrite-b",
                    options: (
                        _ctx: CommonPluginContext,
                        args: OptionsArgs,
                    ): Options | null | void => ({
                        ...args.options,
                        code: "const b = 2;",
                    }),
                },
                {
                    name: "passthrough",
                    options: (
                        _ctx: CommonPluginContext,
                        _args: OptionsArgs,
                    ): null => null,
                },
                {
                    name: "appender",
                    options: (
                        _ctx: CommonPluginContext,
                        args: OptionsArgs,
                    ): Options | null | void => ({
                        ...args.options,
                        code: `${args.options.code}; const c = 3;`,
                    }),
                },
            ],
        });

        expect(result.code).toContain("const b = 2");
        expect(result.code).toContain("const c = 3");
    });

    it("propagates returned options bags across plugins", async (): Promise<void> => {
        // Each plugin returns the bag it wants; the second plugin observes
        // the first return and rewrites the file.
        const seen: Array<string> = [];

        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                {
                    name: "rewrite-code",
                    options: (
                        _ctx: CommonPluginContext,
                        args: OptionsArgs,
                    ): Options | null | void => ({
                        ...args.options,
                        code: "const b = 2;",
                    }),
                },
                {
                    name: "rewrite-file",
                    options: (
                        _ctx: CommonPluginContext,
                        args: OptionsArgs,
                    ): Options | null | void => {
                        seen.push(args.options.code ?? "");
                        seen.push(args.options.cwd ?? "");

                        return {
                            ...args.options,
                            file: "renamed.ts",
                        };
                    },
                },
                {
                    name: "observe",
                    prepare: (ctx: PluginContext): void => {
                        seen.push(ctx.module.file);
                        seen.push(ctx.module.code);
                    },
                },
            ],
        });

        expect(seen).toEqual([
            "const b = 2;",
            "/repo",
            "renamed.ts",
            "const b = 2;",
        ]);
        expect(result.code).toBe("const b = 2;");
    });

    it("applies a returned bag as full-bag replace with defaults", async (): Promise<void> => {
        // Fields the returned bag omits fall back to their defaults; a bag
        // carrying only `code` compiles with the default file `index.js`.
        const seen: Array<string> = [];

        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                {
                    name: "replace-bag",
                    options: (
                        _ctx: CommonPluginContext,
                        args: OptionsArgs,
                    ): Options | null | void => {
                        expect(_ctx.state).toBeInstanceOf(Map);
                        seen.push("has-state");
                        seen.push(args.options.file ?? "");

                        return { code: "const replaced = 1;" };
                    },
                    prepare: (ctx: PluginContext): void => {
                        seen.push(ctx.module.file);
                    },
                },
            ],
        });

        expect(seen).toEqual(["has-state", "index.ts", "index.js"]);
        expect(result.map.sources).toEqual(["index.js"]);
        expect(result.code).toBe("const replaced = 1;");
    });

    it("defaults an omitted cwd to the process working directory", async (): Promise<void> => {
        // `cwd` is optional in `CompileOptions`; when omitted, the core
        // resolves it to the runtime process's working directory before any
        // plugin hook runs, so hooks always observe it as a string. The
        // resolved value is runtime-dependent (the native binding reports
        // Node's `process.cwd()` while the WASI runtime starts at `/`) —
        // pin each runtime's own default.
        const isWasi: boolean = process.env.NAPI_RS_FORCE_WASI === "error";
        const expected: string = isWasi ? "/" : process.cwd();
        const seen: Array<string> = [];

        const result: CompileResult = await compile({
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                {
                    name: "default-cwd",
                    options: (
                        _ctx: CommonPluginContext,
                        args: OptionsArgs,
                    ): null => {
                        seen.push(args.options.cwd ?? "");

                        return null;
                    },
                },
            ],
        });

        expect(seen).toEqual([expected]);
        expect(result.code).toBe("const a = 1;");
    });

    it("re-resolves a cwd omitted by a late options hook to the process working directory", async (): Promise<void> => {
        const isWasi: boolean = process.env.NAPI_RS_FORCE_WASI === "error";
        const expected: string = isWasi ? "/" : process.cwd();
        const seen: Array<string> = [];

        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                {
                    name: "drop-cwd",
                    options: (
                        _ctx: CommonPluginContext,
                        args: OptionsArgs,
                    ): Options => {
                        return { ...args.options, cwd: void 0 };
                    },
                },
                {
                    name: "observe-compile-start-cwd",
                    compileStart: (
                        _ctx: PluginContext,
                        args: CompileStartArgs,
                    ): void => {
                        seen.push(args.options.cwd);
                    },
                },
            ],
        });

        expect(seen).toEqual([expected]);
        expect(result.code).toBe("const a = 1;");
    });

    it("awaits an async options hook and applies the returned bag", async (): Promise<void> => {
        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                {
                    name: "async-options",
                    options: async (
                        _ctx: CommonPluginContext,
                        args: OptionsArgs,
                    ): Promise<Options> => {
                        await new Promise<void>((resolve): void => {
                            setTimeout(resolve, 0);
                        });

                        return {
                            ...args.options,
                            code: "const replaced = 2;",
                        };
                    },
                },
            ],
        });

        expect(result.code).toBe("const replaced = 2;");
    });

    it("injects a plugin through a returned options bag", async (): Promise<void> => {
        // The options hook replaces the bag incl. `plugins`: the injected
        // plugin is wrapped, joins the settled list, and runs like any
        // entry plugin. The injection appends only once, so the fixpoint
        // converges.
        let isInjected: boolean = false;
        const seen: Array<string> = [];

        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                {
                    name: "injector",
                    options: (
                        _ctx: CommonPluginContext,
                        args: OptionsArgs,
                    ): Options | null | void => {
                        if (isInjected) {
                            return null;
                        }
                        isInjected = true;

                        return {
                            ...args.options,
                            plugins: [
                                ...(args.options.plugins ?? []),
                                {
                                    name: "injected",
                                    prepare: (): PrepareResult | null => {
                                        seen.push("injected.prepare");

                                        return { code: "const injected = 3;" };
                                    },
                                },
                            ],
                        };
                    },
                },
            ],
        });

        expect(seen).toEqual(["injected.prepare"]);
        expect(result.code).toBe("const injected = 3;");
    });

    it("runs the injected plugin's options hook in the next fixpoint pass", async (): Promise<void> => {
        let isInjected: boolean = false;
        const seen: Array<string> = [];

        await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                {
                    name: "injector-observer",
                    options: (
                        ctx: CommonPluginContext,
                        args: OptionsArgs,
                    ): Options | null | void => {
                        seen.push(
                            `in:${String(ctx.state.get("injected-ran") === true)}`,
                        );

                        if (isInjected) {
                            return null;
                        }
                        isInjected = true;

                        const next: Options = {
                            ...args.options,
                            code: "const replaced = 2;",
                            plugins: [
                                ...(args.options.plugins ?? []),
                                {
                                    name: "late",
                                    options: (
                                        lateCtx: CommonPluginContext,
                                        _lateArgs: OptionsArgs,
                                    ): void => {
                                        lateCtx.state.set("injected-ran", true);
                                        seen.push("late.options");
                                    },
                                },
                            ],
                        };

                        return next;
                    },
                },
            ],
        });

        // Pass 1 runs the injector, which appends `late`; pass 2 runs the
        // injector again (before `late`, so `injected-ran` is still unset)
        // and then `late`, whose options hook writes the state marker. The
        // fixpoint then converges: no further pass runs the injector after
        // `late`, so `in` is never observed as `true`.
        expect(seen).toEqual(["in:false", "in:false", "late.options"]);
    });

    it("rejects on hook errors with plugin context", async (): Promise<void> => {
        await expect(
            compile({
                cwd: "/repo",
                file: "index.ts",
                code: "const a = 1;",
                plugins: [
                    {
                        name: "boom",
                        transform: (): never => {
                            throw new Error("kaboom");
                        },
                    },
                ],
            }),
        ).rejects.toThrow("kaboom");
    });

    it("rejects a plugin with a missing name", async (): Promise<void> => {
        const nameless: unknown = {
            prepare: (): void => {},
        };

        const build = async (): Promise<CompileResult> =>
            await compile({
                cwd: "/repo",
                file: "index.ts",
                code: "const a = 1;",
                plugins: [nameless as Plugin],
            });

        await expect(build()).rejects.toThrow(TypeError);
        await expect(build()).rejects.toThrow(
            "plugin.name must be a non-empty string",
        );
    });

    it("rejects a plugin with an empty name", async (): Promise<void> => {
        const empty: unknown = {
            name: "",
            prepare: (): void => {},
        };

        const build = async (): Promise<CompileResult> =>
            await compile({
                cwd: "/repo",
                file: "index.ts",
                code: "const a = 1;",
                plugins: [empty as Plugin],
            });

        await expect(build()).rejects.toThrow(TypeError);
        await expect(build()).rejects.toThrow(
            "plugin.name must be a non-empty string",
        );
    });

    it("rejects on an async hook rejection", async (): Promise<void> => {
        await expect(
            compile({
                cwd: "/repo",
                file: "index.ts",
                code: "const a = 1;",
                plugins: [
                    {
                        name: "boom-async",
                        transform: async (): Promise<never> => {
                            throw new Error("async-kaboom");
                        },
                    },
                ],
            }),
        ).rejects.toThrow("async-kaboom");
    });

    it("rejects on an error thrown from prepare", async (): Promise<void> => {
        await expect(
            compile({
                cwd: "/repo",
                file: "index.ts",
                code: "const a = 1;",
                plugins: [
                    {
                        name: "boom-prepare",
                        prepare: (): never => {
                            throw new Error("prepare-kaboom");
                        },
                    },
                ],
            }),
        ).rejects.toThrow("prepare-kaboom");
    });

    it("rejects on an error thrown from finalize", async (): Promise<void> => {
        await expect(
            compile({
                cwd: "/repo",
                file: "index.ts",
                code: "const a = 1;",
                plugins: [
                    {
                        name: "boom-finalize",
                        finalize: (): never => {
                            throw new Error("finalize-kaboom");
                        },
                    },
                ],
            }),
        ).rejects.toThrow("finalize-kaboom");
    });

    it("rejects on an error thrown from options", async (): Promise<void> => {
        await expect(
            compile({
                cwd: "/repo",
                file: "index.ts",
                code: "const a = 1;",
                plugins: [
                    {
                        name: "boom-options",
                        options: (): never => {
                            throw new Error("options-kaboom");
                        },
                    },
                ],
            }),
        ).rejects.toThrow("options-kaboom");
    });

    it("treats a non-function hook value as absent", async (): Promise<void> => {
        const broken: unknown = {
            name: "broken-shape",
            transform: "nope",
        };

        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [broken as Plugin],
        });

        expect(result.code).toBe("const a = 1;");
        expect(result.map.mappings.length).toBeGreaterThan(0);
    });

    it("runs async prepare, transform, and finalize hooks", async (): Promise<void> => {
        const seen: Array<string> = [];

        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                {
                    name: "async-all",
                    options: (
                        _ctx: CommonPluginContext,
                        args: OptionsArgs,
                    ): Options | null | void => {
                        seen.push("options");

                        return {
                            ...args.options,
                            code: "const b = 2;",
                        };
                    },
                    prepare: async (ctx: PluginContext): Promise<void> => {
                        seen.push("prepare");
                        ctx.state.set("marker", "from-async-prepare");
                    },
                    transform: async (ctx: PluginContext): Promise<void> => {
                        seen.push("transform");
                        seen.push(String(ctx.state.get("marker")));
                    },
                    finalize: async (ctx: PluginContext): Promise<void> => {
                        seen.push("finalize");
                        seen.push(String(ctx.state.get("marker")));
                    },
                },
            ],
        });

        expect(result.code).toBe("const b = 2;");
        expect(seen).toEqual([
            "options",
            "prepare",
            "transform",
            "from-async-prepare",
            "finalize",
            "from-async-prepare",
        ]);
    });

    it("orders prepare, transform, and finalize across plugins", async (): Promise<void> => {
        const seen: Array<string> = [];

        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                {
                    name: "first",
                    prepare: (): void => {
                        seen.push("first.prepare");
                    },
                    transform: (_ctx: PluginContext): void => {
                        seen.push("first.transform");
                    },
                    finalize: (): void => {
                        seen.push("first.finalize");
                    },
                },
                {
                    name: "second",
                    prepare: (): void => {
                        seen.push("second.prepare");
                    },
                    transform: (_ctx: PluginContext): void => {
                        seen.push("second.transform");
                    },
                    finalize: (): void => {
                        seen.push("second.finalize");
                    },
                },
            ],
        });

        expect(result.code).toBe("const a = 1;");
        expect(seen).toEqual([
            "first.prepare",
            "second.prepare",
            "first.transform",
            "second.transform",
            "first.finalize",
            "second.finalize",
        ]);
    });

    it("chains transforms return value to return value across plugins", async (): Promise<void> => {
        // oxlint-disable-next-line unicorn/consistent-function-scoping
        const rename = (
            name: string,
        ): ((ctx: PluginContext, args: TransformArgs) => TransformResult) => {
            return (
                _ctx: PluginContext,
                args: TransformArgs,
            ): TransformResult => {
                const program: ProgramFixture =
                    args.ast as unknown as ProgramFixture;
                if (
                    name === "second" &&
                    program.body[0]?.declarations[0]?.id.name !== "first"
                ) {
                    throw new Error("expected the first rename to be visible");
                }
                renameRootIdentifier({ program, name });

                return { ast: args.ast };
            };
        };

        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                { name: "rename-a", transform: rename("first") },
                { name: "rename-b", transform: rename("second") },
            ],
        });

        expect(result.code).toBe("const second = 1;");
    });

    it("propagates state writes from transform to finalize", async (): Promise<void> => {
        const seen: Array<unknown> = [];

        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                {
                    name: "transform-writer",
                    transform: (ctx: PluginContext): void => {
                        ctx.state.set("marker", "from-transform");
                    },
                },
                {
                    name: "finalize-reader",
                    finalize: (ctx: PluginContext): void => {
                        seen.push(ctx.state.get("marker"));
                    },
                },
            ],
        });

        expect(result.code).toBe("const a = 1;");
        expect(seen).toEqual(["from-transform"]);
    });

    it("isolates state across concurrent compiles", async (): Promise<void> => {
        const seen: Array<unknown> = [];

        const compileWith = async (name: string): Promise<CompileResult> =>
            await compile({
                cwd: "/repo",
                file: "index.ts",
                code: "const a = 1;",
                plugins: [
                    {
                        name: "writer",
                        prepare: (ctx: PluginContext): void => {
                            ctx.state.set("marker", name);
                        },
                    },
                    {
                        name: "reader",
                        transform: (): void => void 0,
                    },
                    {
                        name: "verifier",
                        finalize: (ctx: PluginContext): void => {
                            seen.push(ctx.state.get("marker"));
                        },
                    },
                ],
            });

        const [first, second]: [CompileResult, CompileResult] =
            await Promise.all([compileWith("first"), compileWith("second")]);

        expect(first.code).toBe("const a = 1;");
        expect(second.code).toBe("const a = 1;");
        expect(seen).toHaveLength(2);
        expect(seen).toEqual(expect.arrayContaining(["first", "second"]));
    });

    it("passes hook contexts with cwd, module, and code to each hook", async (): Promise<void> => {
        const seen: Array<{ cwd: string; file: string; code: string }> = [];

        const record = (ctx: PluginContext, code: string): void => {
            seen.push({
                cwd: ctx.cwd,
                file: ctx.module.file,
                code,
            });
        };

        const first: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                {
                    name: "fidelity",
                    prepare: (ctx: PluginContext, args: PrepareArgs): void => {
                        record(ctx, args.code);
                    },
                    transform: (ctx: PluginContext): void => {
                        record(ctx, ctx.module.code);
                    },
                    finalize: (
                        ctx: PluginContext,
                        args: FinalizeArgs,
                    ): void => {
                        record(ctx, args.code);
                    },
                },
            ],
        });

        expect(first.code).toBe("const a = 1;");
        expect(seen).toEqual([
            { cwd: "/repo", file: "index.ts", code: "const a = 1;" },
            { cwd: "/repo", file: "index.ts", code: "const a = 1;" },
            { cwd: "/repo", file: "index.ts", code: "const a = 1;" },
        ]);

        const second: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                {
                    name: "options-rewrite",
                    options: (
                        _ctx: CommonPluginContext,
                        args: OptionsArgs,
                    ): Options | null | void => ({
                        ...args.options,
                        code: "const b = 2;",
                    }),
                },
                {
                    name: "fidelity",
                    prepare: (ctx: PluginContext, args: PrepareArgs): void => {
                        record(ctx, args.code);
                    },
                    transform: (ctx: PluginContext): void => {
                        record(ctx, ctx.module.code);
                    },
                    finalize: (
                        ctx: PluginContext,
                        args: FinalizeArgs,
                    ): void => {
                        record(ctx, args.code);
                    },
                },
            ],
        });

        expect(second.code).toBe("const b = 2;");
        expect(seen.slice(3)).toEqual([
            { cwd: "/repo", file: "index.ts", code: "const b = 2;" },
            { cwd: "/repo", file: "index.ts", code: "const b = 2;" },
            { cwd: "/repo", file: "index.ts", code: "const b = 2;" },
        ]);
    });

    it("applies a returned ast mutation through json", async (): Promise<void> => {
        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: `const a = "old";`,
            plugins: [
                {
                    name: "nested-mutator",
                    transform: (
                        _ctx: PluginContext,
                        args: TransformArgs,
                    ): TransformResult => {
                        const program: InitFixture =
                            args.ast as unknown as InitFixture;
                        const statement:
                            | InitFixture["body"][number]
                            | undefined = program.body[0];
                        const declaration:
                            | InitFixture["body"][number]["declarations"][number]
                            | undefined = statement?.declarations[0];
                        if (declaration?.init.type !== "Literal") {
                            throw new Error("expected a literal initializer");
                        }
                        declaration.init.value = "new";

                        return { ast: args.ast };
                    },
                },
            ],
        });

        expect(result.code).toContain(`"new"`);
        expect(result.code).not.toContain(`"old"`);
        expect(result.map.mappings.length).toBeGreaterThan(0);
    });

    it("applies an in-place mutation when the mutated tree is returned", async (): Promise<void> => {
        // Return-based contract: mutating `args.ast` in place only flows
        // back into Rust when the hook returns the mutated tree.
        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                {
                    name: "mutate-and-return",
                    transform: (
                        _ctx: PluginContext,
                        args: TransformArgs,
                    ): TransformResult => {
                        const program: ProgramFixture =
                            args.ast as unknown as ProgramFixture;
                        renameRootIdentifier({ program, name: "mutated" });

                        return { ast: args.ast };
                    },
                },
            ],
        });

        expect(result.code).toBe("const mutated = 1;");
    });

    it("treats an in-place mutation without return as a no-op", async (): Promise<void> => {
        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                {
                    name: "mutate-in-place",
                    transform: (
                        _ctx: PluginContext,
                        args: TransformArgs,
                    ): void => {
                        const program: ProgramFixture =
                            args.ast as unknown as ProgramFixture;
                        renameRootIdentifier({ program, name: "not-applied" });
                    },
                },
            ],
        });

        expect(result.code).toBe("const a = 1;");
    });

    it("applies a whole-root replacement returned from transform", async (): Promise<void> => {
        // The returned `{ ast }` is the replacement: a plugin can build a
        // fresh tree (clone-plus-edit) and return it instead of mutating.
        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                {
                    name: "root-replace",
                    transform: (
                        _ctx: PluginContext,
                        args: TransformArgs,
                    ): TransformResult => {
                        const program: ProgramFixture = structuredClone(
                            args.ast,
                        ) as unknown as ProgramFixture;
                        renameRootIdentifier({ program, name: "replaced" });

                        return { ast: program as never };
                    },
                },
            ],
        });

        expect(result.code).toBe("const replaced = 1;");
    });

    it("applies a returned ast to the compile", async (): Promise<void> => {
        // The hook returns a NEW tree without mutating `args.ast`; the
        // returned one is what Rust reads back.
        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                {
                    name: "returned-tree",
                    transform: (
                        _ctx: PluginContext,
                        args: TransformArgs,
                    ): TransformResult => {
                        const program: ProgramFixture = structuredClone(
                            args.ast,
                        ) as unknown as ProgramFixture;
                        renameRootIdentifier({ program, name: "returned" });

                        return { ast: program as never };
                    },
                },
            ],
        });

        expect(result.code).toBe("const returned = 1;");
        expect(result.map.version).toBe(3);
        expect(result.map.mappings.length).toBeGreaterThan(0);
    });

    it("walks and mutates the ast via telarel/walker", async (): Promise<void> => {
        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.tsx",
            code: "const a = <Button>hi</Button>;",
            plugins: [
                {
                    name: "walker-rename",
                    transform: (
                        _ctx: PluginContext,
                        args: TransformArgs,
                    ): TransformResult => {
                        walk(
                            args.ast as unknown as Parameters<typeof walk>[0],
                            {
                                enter(node): void {
                                    if (
                                        node.type === "JSXIdentifier" &&
                                        node.name === "Button"
                                    ) {
                                        node.name = "Buttonx";
                                    }
                                },
                            },
                        );

                        return { ast: args.ast };
                    },
                },
            ],
        });

        expect(result.code).toContain("<Buttonx>");
    });

    it("rejects cleanly on a non-Error throwable from transform", async (): Promise<void> => {
        await expect(
            compile({
                cwd: "/repo",
                file: "index.ts",
                code: "const a = 1;",
                plugins: [
                    {
                        name: "boom-string",
                        transform: async (): Promise<never> => {
                            // oxlint-disable-next-line typescript/only-throw-error
                            throw "boom-string";
                        },
                    },
                ],
            }),
        ).rejects.toThrow("boom-string");

        // Pinned observed message: the object payload is not propagated —
        // the bridge renders any non-Error throwable as `GenericFailure`,
        // while the plugin name stays visible in the wrapper prefix.
        await expect(
            compile({
                cwd: "/repo",
                file: "index.ts",
                code: "const a = 1;",
                plugins: [
                    {
                        name: "boom-object",
                        transform: async (): Promise<never> => {
                            // oxlint-disable-next-line typescript/only-throw-error
                            throw { reason: "boom-object" };
                        },
                    },
                ],
            }),
        ).rejects.toThrow(
            "transform hook: `boom-object` transform: GenericFailure",
        );
    });

    it("rejects with the late error after a prior plugin replaced the ast", async (): Promise<void> => {
        // oxlint-disable-next-line unicorn/consistent-function-scoping
        const build = async (): Promise<CompileResult> =>
            await compile({
                cwd: "/repo",
                file: "index.ts",
                code: "const a = 1;",
                plugins: [
                    {
                        name: "rename",
                        transform: (
                            _ctx: PluginContext,
                            args: TransformArgs,
                        ): TransformResult => {
                            const program: ProgramFixture =
                                args.ast as unknown as ProgramFixture;
                            renameRootIdentifier({ program, name: "renamed" });

                            return { ast: args.ast };
                        },
                    },
                    {
                        name: "boom-late",
                        transform: async (): Promise<never> => {
                            throw new Error("late-kaboom");
                        },
                    },
                ],
            });

        await expect(build()).rejects.toThrow("late-kaboom");
    });

    it("preserves non-string state values across hooks", async (): Promise<void> => {
        const seen: Array<unknown> = [];
        const seenObject: Array<unknown> = [];

        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                {
                    name: "writer",
                    prepare: (ctx: PluginContext): void => {
                        ctx.state.set("count", 42);
                        ctx.state.set("obj", { nested: true });
                    },
                },
                {
                    name: "reader",
                    transform: (ctx: PluginContext): void => {
                        seen.push(ctx.state.get("count"));
                        seenObject.push(ctx.state.get("obj"));
                    },
                },
            ],
        });

        expect(result.code).toBe("const a = 1;");
        expect(seen).toEqual([42]);
        expect(seenObject).toEqual([{ nested: true }]);
    });

    it("invokes compileStart with the resolved options", async (): Promise<void> => {
        const seen: Array<ResolvedOptions> = [];
        const seenFiles: Array<string> = [];

        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                {
                    name: "first",
                    prepare: (): void => void 0,
                },
                {
                    name: "observe",
                    transform: (): void => void 0,
                    compileStart: (
                        ctx: PluginContext,
                        args: CompileStartArgs,
                    ): void => {
                        seen.push(args.options);
                        seenFiles.push(ctx.module.file);
                    },
                },
            ],
        });

        expect(result.code).toBe("const a = 1;");
        expect(seen).toHaveLength(1);
        expect(seenFiles).toEqual(["index.ts"]);
        // Scalars are fully resolved: language inferred from the extension,
        // sourceType resolved from the language.
        const options: ResolvedOptions = seen[0] as ResolvedOptions;
        expect(options.language).toBe("ts");
        expect(options.sourceType).toBe("unambiguous");
        expect(options.file).toBe("index.ts");
        expect(options.code).toBe("const a = 1;");
        // The settled plugin-name list preserves order and duplicates.
        expect(options.plugins).toEqual(["first", "observe"]);
    });

    it("resolves the defaulted cwd for compileStart", async (): Promise<void> => {
        const isWasi: boolean = process.env.NAPI_RS_FORCE_WASI === "error";
        const expected: string = isWasi ? "/" : process.cwd();
        const seen: Array<string> = [];

        await compile({
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                {
                    name: "observe",
                    compileStart: (
                        _ctx: PluginContext,
                        args: CompileStartArgs,
                    ): void => {
                        seen.push(args.options.cwd);
                    },
                },
            ],
        });

        expect(seen).toEqual([expected]);
    });

    it("passes the compile context to compileStart with the resolved module info", async (): Promise<void> => {
        const seen: Array<{
            cwd: string;
            language: string;
            sourceType: string;
        }> = [];

        await compile({
            cwd: "/repo",
            file: "index.cjs",
            code: "module.exports = 1;",
            plugins: [
                {
                    name: "observe",
                    compileStart: (ctx: PluginContext): void => {
                        seen.push({
                            cwd: ctx.cwd,
                            language: String(ctx.module.language),
                            sourceType: String(ctx.module.sourceType),
                        });
                    },
                },
            ],
        });

        expect(seen).toEqual([
            { cwd: "/repo", language: "js", sourceType: "commonjs" },
        ]);
    });

    it("ignores a garbage return value from compileStart", async (): Promise<void> => {
        const seen: Array<string> = [];

        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                {
                    name: "notify",
                    compileStart: (
                        _ctx: PluginContext,
                        _args: CompileStartArgs,
                    ): void | Promise<void> => {
                        seen.push("started");

                        return void 0;
                    },
                },
            ],
        });

        expect(seen).toEqual(["started"]);
        expect(result.code).toBe("const a = 1;");
        expect(result.map.sources).toEqual(["index.ts"]);
    });

    it("invokes compileEnd on the success path with the compiled output", async (): Promise<void> => {
        const seen: Array<{
            code: string;
            map: unknown;
            err: Error | undefined;
        }> = [];
        const seenFiles: Array<string> = [];

        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                {
                    name: "observe",
                    compileEnd: (
                        ctx: PluginContext,
                        args: CompileEndArgs,
                    ): void => {
                        seen.push({
                            code: args.code,
                            map: args.map,
                            err: args.err,
                        });
                        seenFiles.push(ctx.module.file);
                    },
                },
            ],
        });

        expect(result.code).toBe("const a = 1;");
        expect(seen).toHaveLength(1);
        expect(seenFiles).toEqual(["index.ts"]);
        expect(seen[0]?.code).toBe("const a = 1;");
        expect(seen[0]?.err).toBeUndefined();

        // The final map is the v3 object (parsed from the JSON transport).
        const map: SourceMap = seen[0]?.map as SourceMap;
        expect(map.version).toBe(3);
        expect(map.mappings.length).toBeGreaterThan(0);
    });

    it("invokes compileEnd on the error path with the last good state", async (): Promise<void> => {
        const seen: Array<{ code: string; map: unknown; err: string }> = [];

        const build = async (): Promise<CompileResult> =>
            await compile({
                cwd: "/repo",
                file: "index.ts",
                code: "const a = 1;",
                plugins: [
                    {
                        name: "observer",
                        compileEnd: (
                            _ctx: PluginContext,
                            args: CompileEndArgs,
                        ): void => {
                            seen.push({
                                code: args.code,
                                map: args.map,
                                err: String(args.err),
                            });
                        },
                    },
                    {
                        name: "boom",
                        transform: (): never => {
                            throw new Error("kaboom");
                        },
                    },
                ],
            });

        await expect(build()).rejects.toThrow("kaboom");

        // compileEnd always runs: on the error path it carries the
        // stage-wrapped error and the last good code (the untouched source;
        // no map yet).
        expect(seen).toHaveLength(1);
        expect(seen[0]?.err).toContain("kaboom");
        expect(seen[0]?.err).toContain("transform hook");
        expect(seen[0]?.code).toBe("const a = 1;");
        expect(seen[0]?.map).toBeNull();
    });

    it("replaces the source through a returned prepare result", async (): Promise<void> => {
        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                {
                    name: "prepare-rewrite",
                    prepare: (ctx: PluginContext): PrepareResult => {
                        ctx.state.set("stage", "prepare");

                        return { code: "const b = 2;" };
                    },
                    transform: (
                        _ctx: PluginContext,
                        args: TransformArgs,
                    ): TransformResult => {
                        // Return the received tree (no mutation): a
                        // declared `transform` forces the parse path so the
                        // prepare-returned code is the parse input.
                        return { ast: args.ast };
                    },
                },
            ],
        });

        expect(result.code).toBe("const b = 2;");
    });

    it("keeps the source when a prepare hook returns null", async (): Promise<void> => {
        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                {
                    name: "prepare-null",
                    prepare: (): null => null,
                    transform: (): void => void 0,
                },
            ],
        });

        expect(result.code).toBe("const a = 1;");
    });

    it("keeps the source when a prepare hook returns undefined", async (): Promise<void> => {
        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                {
                    name: "prepare-void",
                    prepare: (): void => void 0,
                    transform: (): void => void 0,
                },
            ],
        });

        expect(result.code).toBe("const a = 1;");
    });

    it("applies a returned prepare map to the final output map", async (): Promise<void> => {
        // The prepare hook deletes the original's first line and returns its
        // incremental map (dst(0,0) -> src(1,0), encoded `AAAC`): the
        // composed output map resolves the parse input back to the
        // ORIGINAL source line 1, so the mappings must be `AAAC`, not the
        // plain identity `AAAA` (the composed map is NOT the raw identity).
        const original: string = "const a = 1;\nlet b = 2;";

        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: original,
            plugins: [
                {
                    name: "prepare-with-map",
                    prepare: (): PrepareResult => ({
                        code: "let b = 2;",
                        map: {
                            version: 3,
                            mappings: "AAAC",
                            sources: ["index.ts"],
                            names: [],
                            sourcesContent: ["let b = 2;"],
                        },
                    }),
                },
            ],
        });

        expect(result.code).toBe("let b = 2;");
        expect(result.map.mappings).toBe("AAAC");
        // The composed map carries the ORIGINAL source list, not the
        // prepare-returned one.
        expect(result.map.sources).toEqual(["index.ts"]);
    });

    it("keeps the identity map when a prepare hook omits its map", async (): Promise<void> => {
        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "let b = 2;",
            plugins: [
                {
                    name: "prepare-no-map",
                    prepare: (): PrepareResult => ({ code: "let c = 3;" }),
                },
            ],
        });

        expect(result.code).toBe("let c = 3;");
        // No map returned: the composition is skipped and the identity map
        // over the parse input carries through.
        expect(result.map.mappings).toBe("AAAA");
    });

    it("carries a prepare rewrite into the next prepare hook", async (): Promise<void> => {
        // The `prepare` fold is carried: the second hook must receive the
        // first's returned code, not the original source. A fixed-input
        // bridge would record `"const original = 1;"` here.
        const seen: Array<string> = [];

        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const original = 1;",
            plugins: [
                {
                    name: "prepare-first",
                    prepare: (): PrepareResult => ({
                        code: "const injected = 1;",
                    }),
                },
                {
                    name: "prepare-probe",
                    prepare: (_ctx: PluginContext, args: PrepareArgs): void => {
                        seen.push(args.code);
                    },
                },
            ],
        });

        expect(seen).toEqual(["const injected = 1;"]);
        expect(result.code).toBe("const injected = 1;");
    });

    it("replaces the output through a returned finalize result", async (): Promise<void> => {
        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                {
                    name: "force-parse",
                    transform: (): void => void 0,
                },
                {
                    name: "finalize-rewrite",
                    finalize: (ctx: PluginContext): FinalizeResult => {
                        ctx.state.set("stage", "finalize");

                        return { code: "const replaced = 1;" };
                    },
                },
            ],
        });

        expect(result.code).toBe("const replaced = 1;");
    });

    it("applies a returned finalize map to the final output map", async (): Promise<void> => {
        // The finalize hook rewrites the generated code and returns a
        // single-token incremental map: the finalize map owns the OUTPUT dst
        // positions (composed with the codegen map), so the final mappings
        // carry the finalize map's single dst token instead of the multi-token
        // codegen map.
        const baseline: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "console.log(1);",
            plugins: [{ name: "force-parse", transform: (): void => void 0 }],
        });

        expect(baseline.map.mappings).not.toBe("AAAA");

        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "console.log(1);",
            plugins: [
                { name: "force-parse", transform: (): void => void 0 },
                {
                    name: "finalize-with-map",
                    finalize: (): FinalizeResult => ({
                        code: "// banner\nconsole.log(1);",
                        map: {
                            version: 3,
                            mappings: "AAAC",
                            sources: ["index.ts"],
                            names: [],
                            sourcesContent: ["// banner\nconsole.log(1);"],
                        },
                    }),
                },
            ],
        });

        expect(result.code).toBe("// banner\nconsole.log(1);");
        // The single finalize token dst(0,0)->src(1,0): the banner line maps
        // through the codegen map back to the generated line 1.
        expect(result.map.mappings).not.toBe(baseline.map.mappings);
        expect(result.map.mappings.length).toBeGreaterThan(0);
    });

    it("keeps the codegen map when a finalize hook omits its map", async (): Promise<void> => {
        const baseline: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "console.log(1);",
            plugins: [{ name: "force-parse", transform: (): void => void 0 }],
        });

        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "console.log(1);",
            plugins: [
                { name: "force-parse", transform: (): void => void 0 },
                {
                    name: "finalize-no-map",
                    finalize: (): FinalizeResult => ({
                        code: "console.log(2);",
                    }),
                },
            ],
        });

        expect(result.code).toContain("console.log(2);");
        // No finalize map returned: the codegen map over the original parse
        // input carries through UNCHANGED.
        expect(result.map.mappings).toBe(baseline.map.mappings);
    });

    it("carries a finalize rewrite into the next finalize hook", async (): Promise<void> => {
        // The `finalize` fold is carried: the second hook must receive the
        // first's returned code, not the generated code. A fixed-input
        // bridge would record `"const a = 1;"` here.
        const seen: Array<string> = [];

        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                { name: "force-parse", transform: (): void => void 0 },
                {
                    name: "finalize-first",
                    finalize: (): FinalizeResult => ({
                        code: "const injected = 1;",
                    }),
                },
                {
                    name: "finalize-probe",
                    finalize: (
                        _ctx: PluginContext,
                        args: FinalizeArgs,
                    ): void => {
                        seen.push(args.code);
                    },
                },
            ],
        });

        expect(seen).toEqual(["const injected = 1;"]);
        expect(result.code).toBe("const injected = 1;");
    });

    it("reuses the same plugin object across compiles with per-compile state", async (): Promise<void> => {
        // The SAME plugin objects run across five sequential compiles, but
        // each `compile()` builds a fresh `state` Map: the marker written by
        // the first compile must not leak into the later ones.
        const seen: Array<unknown> = [];
        let shouldWrite: boolean = true;

        const writer: Plugin = {
            name: "loop-writer",
            prepare: (ctx: PluginContext): void => {
                if (shouldWrite) {
                    ctx.state.set("marker", "loop");
                }
            },
        };
        const reader: Plugin = {
            name: "loop-reader",
            transform: (ctx: PluginContext): void => {
                seen.push(ctx.state.get("marker") ?? "__NO_MARKER__");
            },
        };

        const results: Array<CompileResult> = [];
        for (let index: number = 0; index < 5; index += 1) {
            const result: CompileResult = await compile({
                cwd: "/repo",
                file: "index.ts",
                code: "const a = 1;",
                plugins: [writer, reader],
            });
            results.push(result);
            shouldWrite = false;
        }

        expect(results).toHaveLength(5);

        for (const result of results) {
            expect(result.code).toBe("const a = 1;");
            expect(result.map.mappings.length).toBeGreaterThan(0);
        }

        expect(seen).toEqual([
            "loop",
            "__NO_MARKER__",
            "__NO_MARKER__",
            "__NO_MARKER__",
            "__NO_MARKER__",
        ]);
    });

    it("runs concurrent compiles with mixed sync and async hooks", async (): Promise<void> => {
        const seen: Array<string> = [];

        const compiles: Array<Promise<CompileResult>> = [];
        for (let index: number = 0; index < 8; index += 1) {
            const marker: string = String(index);
            const isOdd: boolean = index % 2 === 1;
            compiles.push(
                compile({
                    cwd: "/repo",
                    file: "index.ts",
                    code: "const a = 1;",
                    plugins: [
                        {
                            name: `mixed-${marker}`,
                            prepare: isOdd
                                ? async (ctx: PluginContext): Promise<void> => {
                                      ctx.state.set("id", marker);
                                  }
                                : (ctx: PluginContext): void => {
                                      ctx.state.set("id", marker);
                                  },
                            transform: isOdd
                                ? (ctx: PluginContext): void => {
                                      seen.push(String(ctx.state.get("id")));
                                  }
                                : async (ctx: PluginContext): Promise<void> => {
                                      seen.push(String(ctx.state.get("id")));
                                  },
                        },
                    ],
                }),
            );
        }

        const results: Array<CompileResult> = await Promise.all(compiles);

        expect(results).toHaveLength(8);
        for (const result of results) expect(result.code).toBe("const a = 1;");
        expect(seen).toHaveLength(8);
        expect(seen).toEqual(
            expect.arrayContaining(["0", "1", "2", "3", "4", "5", "6", "7"]),
        );
    });
});
