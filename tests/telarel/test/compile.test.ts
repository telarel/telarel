import type {
    CodeResult,
    CommonPluginContext,
    EndArgs,
    FinalizeArgs,
    OptionsArgs,
    Plugin,
    PluginContext,
    PrepareArgs,
    ResolvedOptions,
    StartArgs,
    TransformArgs,
    TransformResult,
} from "telarel";

import type { ProgramFixture } from "#/functions/ast";

import { Ast, compile } from "telarel";
import { walk } from "telarel/walker";
import { describe, expect, it } from "vitest";

import { renameRootIdentifier } from "#/functions/ast";
import { buildMap } from "#/functions/sourcemap";

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
        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
        });

        expect(result.code).toBe("const a = 1;");
        expect(result.map?.version).toBe(3);
        expect(result.map?.mappings.length).toBeGreaterThan(0);
    });

    it("defaults the file to index.ts with the ts grammar", async (): Promise<void> => {
        // The default file is `index.ts`, so the default grammar is ts: an
        // annotated declaration parses and round-trips without a transform.
        const result: CodeResult = await compile("const a: number = 1;");

        expect(result.code).toBe("const a: number = 1;");
        expect(result.map?.sources).toEqual(["index.ts"]);
    });

    it("honors explicit file and language overrides", async (): Promise<void> => {
        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "entry.jsx",
            language: "jsx",
        });

        expect(result.map?.sources).toEqual(["entry.jsx"]);
    });

    it("treats a pass-through transform hook as no change", async (): Promise<void> => {
        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "pass-through",
                    transform: (): void => void 0,
                },
            ],
        });

        expect(result.code).toBe("const a = 1;");
        expect(result.map?.version).toBe(3);
        expect(result.map?.mappings.length).toBeGreaterThan(0);
    });

    it("runs a visitor from oxc-parser inside transform", async (): Promise<void> => {
        const seen: Array<string> = [];

        const result: CodeResult = await compile("console.log(1);", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "spy",
                    transform: (
                        _ctx: PluginContext,
                        args: TransformArgs,
                    ): void => {
                        const program: CallFixture = args.ast
                            .program as unknown as CallFixture;
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
        expect(result.map?.version).toBe(3);
        expect(result.map?.mappings.length).toBeGreaterThan(0);
    });

    it("chains options hooks last-wins on the file", async (): Promise<void> => {
        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "rewrite-b",
                    options: (
                        _ctx: CommonPluginContext,
                        args: OptionsArgs,
                    ): OptionsArgs | null | void => ({
                        options: {
                            ...args.options,
                            file: "b.ts",
                        },
                    }),
                },
                {
                    name: "rewrite-c",
                    options: (
                        _ctx: CommonPluginContext,
                        args: OptionsArgs,
                    ): OptionsArgs | null | void => ({
                        options: {
                            ...args.options,
                            file: "c.ts",
                        },
                    }),
                },
            ],
        });

        // The bag cannot carry a `code` rewrite, so the source is unchanged;
        // the last returned bag wins the file.
        expect(result.code).toBe("const a = 1;");
        expect(result.map?.sources).toEqual(["c.ts"]);
    });

    it("keeps the bag unchanged when an options hook returns null", async (): Promise<void> => {
        const seen: Array<string> = [];

        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "rewrite-b",
                    options: (
                        _ctx: CommonPluginContext,
                        args: OptionsArgs,
                    ): OptionsArgs | null | void => ({
                        options: {
                            ...args.options,
                            file: "b.ts",
                        },
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
                    name: "observer",
                    options: (
                        _ctx: CommonPluginContext,
                        args: OptionsArgs,
                    ): null => {
                        seen.push(args.options.file ?? "");

                        return null;
                    },
                },
            ],
        });

        // `null` = no change: the bag carries `b.ts` into the observer and
        // nothing after it replaces the file.
        expect(seen).toEqual(["b.ts"]);
        expect(result.code).toBe("const a = 1;");
        expect(result.map?.sources).toEqual(["b.ts"]);
    });

    it("propagates returned options bags across plugins", async (): Promise<void> => {
        // Later plugins observe the bag returned by earlier plugins.
        const seen: Array<string> = [];

        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "rewrite-cwd",
                    options: (
                        _ctx: CommonPluginContext,
                        args: OptionsArgs,
                    ): OptionsArgs | null | void => ({
                        options: {
                            ...args.options,
                            cwd: "/other",
                        },
                    }),
                },
                {
                    name: "rewrite-file",
                    options: (
                        _ctx: CommonPluginContext,
                        args: OptionsArgs,
                    ): OptionsArgs | null | void => {
                        seen.push(args.options.file ?? "");
                        seen.push(args.options.cwd ?? "");

                        return {
                            options: {
                                ...args.options,
                                file: "renamed.ts",
                            },
                        };
                    },
                },
                {
                    name: "observe",
                    prepare: (ctx: PluginContext): void => {
                        seen.push(ctx.module.file);
                    },
                },
            ],
        });

        expect(seen).toEqual(["index.ts", "/other", "renamed.ts"]);
        expect(result.code).toBe("const a = 1;");
        expect(result.map?.sources).toEqual(["renamed.ts"]);
    });

    it("applies a returned bag as full-bag replace with defaults", async (): Promise<void> => {
        // Fields the returned bag omits fall back to their defaults. The bag
        // explicitly carries the current `file` and plugin list: omitting
        // `plugins` would wholesale-replace the list to `[]` (the documented
        // `plugins` default) and drop the plugin.
        const seen: Array<string> = [];

        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "replace-bag",
                    options: (
                        _ctx: CommonPluginContext,
                        args: OptionsArgs,
                    ): OptionsArgs | null | void => {
                        seen.push("has-context");
                        seen.push(args.options.file ?? "");

                        return {
                            options: {
                                file: args.options.file,
                                plugins: args.options.plugins,
                            },
                        };
                    },
                    prepare: (ctx: PluginContext): void => {
                        seen.push(ctx.module.file);
                    },
                },
            ],
        });

        expect(seen).toEqual(["has-context", "index.ts", "index.ts"]);
        expect(result.map?.sources).toEqual(["index.ts"]);
        expect(result.code).toBe("const a = 1;");
    });

    it("defaults an omitted cwd to the process working directory", async (): Promise<void> => {
        // `cwd` is optional. The `options` fixpoint runs BEFORE resolution, so
        // an `options` hook observes the raw value (`""` when unset); the core
        // resolves it to the process working directory for `start` and later
        // hooks. The resolved value is runtime-dependent (the native binding
        // reports Node's `process.cwd()` while the WASI runtime starts at `/`)
        // — pin each runtime's own default.
        const isWasi: boolean = process.env.NAPI_RS_FORCE_WASI === "error";
        const expected: string = isWasi ? "/" : process.cwd();
        const seenRaw: Array<string> = [];
        const seenResolved: Array<string> = [];

        const result: CodeResult = await compile("const a = 1;", {
            file: "index.ts",
            plugins: [
                {
                    name: "observe-raw-cwd",
                    options: (
                        _ctx: CommonPluginContext,
                        args: OptionsArgs,
                    ): null => {
                        seenRaw.push(args.options.cwd ?? "");

                        return null;
                    },
                },
                {
                    name: "observe-resolved-cwd",
                    start: (_ctx: PluginContext, args: StartArgs): void => {
                        seenResolved.push(args.options.cwd);
                    },
                },
            ],
        });

        expect(seenRaw).toEqual([""]);
        expect(seenResolved).toEqual([expected]);
        expect(result.code).toBe("const a = 1;");
    });

    it("re-resolves a cwd omitted by a late options hook to the process working directory", async (): Promise<void> => {
        const isWasi: boolean = process.env.NAPI_RS_FORCE_WASI === "error";
        const expected: string = isWasi ? "/" : process.cwd();
        const seen: Array<string> = [];

        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "drop-cwd",
                    options: (
                        _ctx: CommonPluginContext,
                        args: OptionsArgs,
                    ): OptionsArgs => {
                        return {
                            options: { ...args.options, cwd: void 0 },
                        };
                    },
                },
                {
                    name: "observe-start-cwd",
                    start: (_ctx: PluginContext, args: StartArgs): void => {
                        seen.push(args.options.cwd);
                    },
                },
            ],
        });

        expect(seen).toEqual([expected]);
        expect(result.code).toBe("const a = 1;");
    });

    it("awaits an async options hook and applies the returned bag", async (): Promise<void> => {
        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "async-options",
                    options: async (
                        _ctx: CommonPluginContext,
                        args: OptionsArgs,
                    ): Promise<OptionsArgs> => {
                        await new Promise<void>((resolve): void => {
                            setTimeout(resolve, 0);
                        });

                        return {
                            options: {
                                ...args.options,
                                file: "async.ts",
                            },
                        };
                    },
                },
            ],
        });

        expect(result.code).toBe("const a = 1;");
        expect(result.map?.sources).toEqual(["async.ts"]);
    });

    it("injects a plugin through a returned options bag", async (): Promise<void> => {
        // The options hook replaces the bag incl. `plugins`: the injected
        // plugin is wrapped, joins the settled list, and runs like any
        // entry plugin. The injection appends only once, so the fixpoint
        // converges.
        let isInjected: boolean = false;
        const seen: Array<string> = [];

        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "injector",
                    options: (
                        _ctx: CommonPluginContext,
                        args: OptionsArgs,
                    ): OptionsArgs | null | void => {
                        if (isInjected) {
                            return null;
                        }
                        isInjected = true;

                        return {
                            options: {
                                ...args.options,
                                plugins: [
                                    ...(args.options.plugins ?? []),
                                    {
                                        name: "injected",
                                        prepare: (): void => {
                                            seen.push("injected.prepare");
                                        },
                                    },
                                ],
                            },
                        };
                    },
                },
            ],
        });

        expect(seen).toEqual(["injected.prepare"]);
        expect(result.code).toBe("const a = 1;");
    });

    it("runs the injected plugin's options hook in the next fixpoint pass", async (): Promise<void> => {
        let isInjected: boolean = false;
        let lateRan: boolean = false;
        const seen: Array<string> = [];

        await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "injector-observer",
                    options: (
                        _ctx: CommonPluginContext,
                        args: OptionsArgs,
                    ): OptionsArgs | null | void => {
                        seen.push(`in:${String(lateRan)}`);

                        if (isInjected) {
                            return null;
                        }
                        isInjected = true;

                        const next: OptionsArgs = {
                            options: {
                                ...args.options,
                                plugins: [
                                    ...(args.options.plugins ?? []),
                                    {
                                        name: "late",
                                        options: (
                                            _lateCtx: CommonPluginContext,
                                            _lateArgs: OptionsArgs,
                                        ): void => {
                                            lateRan = true;
                                            seen.push("late.options");
                                        },
                                    },
                                ],
                            },
                        };

                        return next;
                    },
                },
            ],
        });

        // The injector runs before `late` on every pass, so it never observes
        // the flag `late` flips.
        expect(seen).toEqual(["in:false", "in:false", "late.options"]);
    });

    it("rejects on hook errors with plugin context", async (): Promise<void> => {
        await expect(
            compile("const a = 1;", {
                cwd: "/repo",
                file: "index.ts",
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

        const build = async (): Promise<CodeResult> =>
            await compile("const a = 1;", {
                cwd: "/repo",
                file: "index.ts",
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

        const build = async (): Promise<CodeResult> =>
            await compile("const a = 1;", {
                cwd: "/repo",
                file: "index.ts",
                plugins: [empty as Plugin],
            });

        await expect(build()).rejects.toThrow(TypeError);
        await expect(build()).rejects.toThrow(
            "plugin.name must be a non-empty string",
        );
    });

    it("rejects on an async hook rejection", async (): Promise<void> => {
        await expect(
            compile("const a = 1;", {
                cwd: "/repo",
                file: "index.ts",
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
            compile("const a = 1;", {
                cwd: "/repo",
                file: "index.ts",
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
            compile("const a = 1;", {
                cwd: "/repo",
                file: "index.ts",
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
            compile("const a = 1;", {
                cwd: "/repo",
                file: "index.ts",
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

        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [broken as Plugin],
        });

        expect(result.code).toBe("const a = 1;");
        expect(result.map?.mappings.length).toBeGreaterThan(0);
    });

    it("runs async prepare, transform, and finalize hooks", async (): Promise<void> => {
        const seen: Array<string> = [];

        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "async-all",
                    options: (
                        _ctx: CommonPluginContext,
                        args: OptionsArgs,
                    ): OptionsArgs | null | void => {
                        seen.push("options");

                        return {
                            options: {
                                ...args.options,
                                file: "b.ts",
                            },
                        };
                    },
                    prepare: async (): Promise<void> => {
                        seen.push("prepare");
                    },
                    transform: async (): Promise<void> => {
                        seen.push("transform");
                    },
                    finalize: async (): Promise<void> => {
                        seen.push("finalize");
                    },
                },
            ],
        });

        expect(result.code).toBe("const a = 1;");
        expect(result.map?.sources).toEqual(["b.ts"]);
        expect(seen).toEqual(["options", "prepare", "transform", "finalize"]);
    });

    it("orders prepare, transform, and finalize across plugins", async (): Promise<void> => {
        const seen: Array<string> = [];

        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
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
                const program: ProgramFixture = args.ast
                    .program as unknown as ProgramFixture;
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

        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                { name: "rename-a", transform: rename("first") },
                { name: "rename-b", transform: rename("second") },
            ],
        });

        expect(result.code).toBe("const second = 1;");
    });

    it("passes hook contexts with cwd and module but no code", async (): Promise<void> => {
        const seen: Array<{ cwd: string; file: string; hasCode: boolean }> = [];

        const record = (ctx: PluginContext): void => {
            seen.push({
                cwd: ctx.cwd,
                file: ctx.module.file,
                hasCode: Object.prototype.hasOwnProperty.call(
                    ctx.module,
                    "code",
                ),
            });
        };

        const first: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "fidelity",
                    prepare: (ctx: PluginContext, _args: PrepareArgs): void => {
                        record(ctx);
                    },
                    transform: (ctx: PluginContext): void => {
                        record(ctx);
                    },
                    finalize: (
                        ctx: PluginContext,
                        _args: FinalizeArgs,
                    ): void => {
                        record(ctx);
                    },
                },
            ],
        });

        expect(first.code).toBe("const a = 1;");
        expect(seen).toEqual([
            { cwd: "/repo", file: "index.ts", hasCode: false },
            { cwd: "/repo", file: "index.ts", hasCode: false },
            { cwd: "/repo", file: "index.ts", hasCode: false },
        ]);

        const second: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "options-rewrite",
                    options: (
                        _ctx: CommonPluginContext,
                        args: OptionsArgs,
                    ): OptionsArgs | null | void => ({
                        options: {
                            ...args.options,
                            file: "renamed.ts",
                        },
                    }),
                },
                {
                    name: "fidelity",
                    prepare: (ctx: PluginContext, _args: PrepareArgs): void => {
                        record(ctx);
                    },
                    transform: (ctx: PluginContext): void => {
                        record(ctx);
                    },
                    finalize: (
                        ctx: PluginContext,
                        _args: FinalizeArgs,
                    ): void => {
                        record(ctx);
                    },
                },
            ],
        });

        expect(second.code).toBe("const a = 1;");
        expect(seen.slice(3)).toEqual([
            { cwd: "/repo", file: "renamed.ts", hasCode: false },
            { cwd: "/repo", file: "renamed.ts", hasCode: false },
            { cwd: "/repo", file: "renamed.ts", hasCode: false },
        ]);
    });

    it("applies a returned ast mutation through json", async (): Promise<void> => {
        const result: CodeResult = await compile(`const a = "old";`, {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "nested-mutator",
                    transform: (
                        _ctx: PluginContext,
                        args: TransformArgs,
                    ): TransformResult => {
                        const program: InitFixture = args.ast
                            .program as unknown as InitFixture;
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
        expect(result.map?.mappings.length).toBeGreaterThan(0);
    });

    it("applies an in-place mutation when the mutated tree is returned", async (): Promise<void> => {
        // Return-based contract: mutating `args.ast` in place only flows
        // back into Rust when the hook returns the mutated tree.
        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "mutate-and-return",
                    transform: (
                        _ctx: PluginContext,
                        args: TransformArgs,
                    ): TransformResult => {
                        const program: ProgramFixture = args.ast
                            .program as unknown as ProgramFixture;
                        renameRootIdentifier({ program, name: "mutated" });

                        return { ast: args.ast };
                    },
                },
            ],
        });

        expect(result.code).toBe("const mutated = 1;");
    });

    it("treats an in-place mutation without return as a no-op", async (): Promise<void> => {
        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "mutate-in-place",
                    transform: (
                        _ctx: PluginContext,
                        args: TransformArgs,
                    ): void => {
                        const program: ProgramFixture = args.ast
                            .program as unknown as ProgramFixture;
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
        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "root-replace",
                    transform: (
                        _ctx: PluginContext,
                        args: TransformArgs,
                    ): TransformResult => {
                        const program: ProgramFixture = structuredClone(
                            args.ast.program,
                        ) as unknown as ProgramFixture;
                        renameRootIdentifier({ program, name: "replaced" });

                        return {
                            ast: new Ast({
                                program: program as never,
                                source: args.ast.source,
                            }),
                        };
                    },
                },
            ],
        });

        expect(result.code).toBe("const replaced = 1;");
    });

    it("applies a returned ast to the compile", async (): Promise<void> => {
        // The hook returns a NEW tree without mutating `args.ast`; the
        // returned one is what Rust reads back.
        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "returned-tree",
                    transform: (
                        _ctx: PluginContext,
                        args: TransformArgs,
                    ): TransformResult => {
                        const program: ProgramFixture = structuredClone(
                            args.ast.program,
                        ) as unknown as ProgramFixture;
                        renameRootIdentifier({ program, name: "returned" });

                        return {
                            ast: new Ast({
                                program: program as never,
                                source: args.ast.source,
                            }),
                        };
                    },
                },
            ],
        });

        expect(result.code).toBe("const returned = 1;");
        expect(result.map?.version).toBe(3);
        expect(result.map?.mappings.length).toBeGreaterThan(0);
    });

    it("walks and mutates the ast via telarel/walker", async (): Promise<void> => {
        const result: CodeResult = await compile(
            "const a = <Button>hi</Button>;",
            {
                cwd: "/repo",
                file: "index.tsx",
                plugins: [
                    {
                        name: "walker-rename",
                        transform: (
                            _ctx: PluginContext,
                            args: TransformArgs,
                        ): TransformResult => {
                            args.ast.mutate((program): void => {
                                walk(program, {
                                    enter(node): void {
                                        if (
                                            node.type === "JSXIdentifier" &&
                                            node.name === "Button"
                                        ) {
                                            node.name = "Buttonx";
                                        }
                                    },
                                });
                            });

                            return { ast: args.ast };
                        },
                    },
                ],
            },
        );

        expect(result.code).toContain("<Buttonx>");
    });

    it("mutates the carried ast through Ast.mutate", async (): Promise<void> => {
        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "mutate-rename",
                    transform: (
                        _ctx: PluginContext,
                        args: TransformArgs,
                    ): TransformResult => {
                        args.ast.mutate((program): void => {
                            renameRootIdentifier({
                                program: program as unknown as ProgramFixture,
                                name: "mutated",
                            });
                        });

                        return { ast: args.ast };
                    },
                },
            ],
        });

        expect(result.code).toBe("const mutated = 1;");
    });

    it("rejects cleanly on a non-Error throwable from transform", async (): Promise<void> => {
        await expect(
            compile("const a = 1;", {
                cwd: "/repo",
                file: "index.ts",
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
        // and the plugin name is no longer added by the wrapper.
        await expect(
            compile("const a = 1;", {
                cwd: "/repo",
                file: "index.ts",
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
        ).rejects.toThrow("GenericFailure");
    });

    it("rejects with the late error after a prior plugin replaced the ast", async (): Promise<void> => {
        // oxlint-disable-next-line unicorn/consistent-function-scoping
        const build = async (): Promise<CodeResult> =>
            await compile("const a = 1;", {
                cwd: "/repo",
                file: "index.ts",
                plugins: [
                    {
                        name: "rename",
                        transform: (
                            _ctx: PluginContext,
                            args: TransformArgs,
                        ): TransformResult => {
                            const program: ProgramFixture = args.ast
                                .program as unknown as ProgramFixture;
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

    it("invokes start with the resolved options", async (): Promise<void> => {
        const seen: Array<ResolvedOptions> = [];
        const seenFiles: Array<string> = [];

        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "first",
                    prepare: (): void => void 0,
                },
                {
                    name: "observe",
                    transform: (): void => void 0,
                    start: (ctx: PluginContext, args: StartArgs): void => {
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
        expect(options.sourceMap).toEqual({ input: null, output: null });
        // The settled plugin-name list preserves order and duplicates.
        expect(options.plugins).toEqual(["first", "observe"]);
    });

    it("resolves the defaulted cwd for start", async (): Promise<void> => {
        const isWasi: boolean = process.env.NAPI_RS_FORCE_WASI === "error";
        const expected: string = isWasi ? "/" : process.cwd();
        const seen: Array<string> = [];

        await compile("const a = 1;", {
            file: "index.ts",
            plugins: [
                {
                    name: "observe",
                    start: (_ctx: PluginContext, args: StartArgs): void => {
                        seen.push(args.options.cwd);
                    },
                },
            ],
        });

        expect(seen).toEqual([expected]);
    });

    it("passes the compile context to start with the resolved module info", async (): Promise<void> => {
        const seen: Array<{
            cwd: string;
            language: string;
            sourceType: string;
        }> = [];

        await compile("module.exports = 1;", {
            cwd: "/repo",
            file: "index.cjs",
            plugins: [
                {
                    name: "observe",
                    start: (ctx: PluginContext): void => {
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

    it("ignores a garbage return value from start", async (): Promise<void> => {
        const seen: Array<string> = [];

        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "notify",
                    start: (
                        _ctx: PluginContext,
                        _args: StartArgs,
                    ): void | Promise<void> => {
                        seen.push("started");

                        return void 0;
                    },
                },
            ],
        });

        expect(seen).toEqual(["started"]);
        expect(result.code).toBe("const a = 1;");
        expect(result.map?.sources).toEqual(["index.ts"]);
    });

    it("invokes end on the success path with a null error", async (): Promise<void> => {
        const seen: Array<string | null> = [];
        const seenFiles: Array<string> = [];

        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "observe",
                    end: (ctx: PluginContext, args: EndArgs): void => {
                        seen.push(args.error);
                        seenFiles.push(ctx.module.file);
                    },
                },
            ],
        });

        expect(result.code).toBe("const a = 1;");
        expect(seen).toEqual([null]);
        expect(seenFiles).toEqual(["index.ts"]);
    });

    it("invokes end on the error path with the first stage error", async (): Promise<void> => {
        const seen: Array<string | null> = [];

        const build = async (): Promise<CodeResult> =>
            await compile("const a = 1;", {
                cwd: "/repo",
                file: "index.ts",
                plugins: [
                    {
                        name: "observer",
                        end: (_ctx: PluginContext, args: EndArgs): void => {
                            seen.push(args.error);
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

        // `end` always runs: on the error path it carries the first stage
        // error, and nothing else.
        expect(seen).toHaveLength(1);
        expect(seen[0]).toContain("kaboom");
    });

    it("observes the parsed AST in prepare", async (): Promise<void> => {
        const seen: Array<{
            keys: Array<string>;
            type: string;
            code: string;
        }> = [];

        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "prepare-observer",
                    prepare: (_ctx: PluginContext, args: PrepareArgs): void => {
                        seen.push({
                            keys: Object.keys(args),
                            type: args.ast.program.type,
                            code: args.ast.source,
                        });
                    },
                },
            ],
        });

        // The prepare payload carries only the parsed AST; there is no
        // rewrite channel, so the return is ignored.
        expect(seen).toEqual([
            { keys: ["ast"], type: "Program", code: "const a = 1;" },
        ]);
        expect(result.code).toBe("const a = 1;");
    });

    it("ignores a return value from a prepare hook", async (): Promise<void> => {
        const plugin: unknown = {
            name: "prepare-ignored",
            prepare: (): unknown => ({ code: "const b = 2;" }),
        };

        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [plugin as Plugin],
        });

        expect(result.code).toBe("const a = 1;");
    });

    it("keeps the source when a prepare hook returns nothing", async (): Promise<void> => {
        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "prepare-void",
                    prepare: (): void => void 0,
                },
            ],
        });

        expect(result.code).toBe("const a = 1;");
    });

    it("observes the final AST in finalize", async (): Promise<void> => {
        const seen: Array<{
            keys: Array<string>;
            type: string;
            code: string;
        }> = [];

        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "finalize-observer",
                    finalize: (
                        _ctx: PluginContext,
                        args: FinalizeArgs,
                    ): void => {
                        seen.push({
                            keys: Object.keys(args),
                            type: args.ast.program.type,
                            code: args.ast.source,
                        });
                    },
                },
            ],
        });

        expect(seen).toEqual([
            { keys: ["ast"], type: "Program", code: "const a = 1;" },
        ]);
        expect(result.code).toBe("const a = 1;");
    });

    it("carries a transform rewrite into the AST observed by finalize", async (): Promise<void> => {
        const names: Array<string> = [];

        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "renamer",
                    transform: (
                        _ctx: PluginContext,
                        args: TransformArgs,
                    ): TransformResult => {
                        const program: ProgramFixture = args.ast
                            .program as unknown as ProgramFixture;
                        renameRootIdentifier({ program, name: "renamed" });

                        return { ast: args.ast };
                    },
                },
                {
                    name: "finalize-observer",
                    finalize: (
                        _ctx: PluginContext,
                        args: FinalizeArgs,
                    ): void => {
                        const program: ProgramFixture = args.ast
                            .program as unknown as ProgramFixture;

                        names.push(
                            program.body[0]?.declarations[0]?.id.name ?? "",
                        );
                    },
                },
            ],
        });

        expect(names).toEqual(["renamed"]);
        expect(result.code).toBe("const renamed = 1;");
    });

    it("keeps the codegen map through prepare and finalize observers", async (): Promise<void> => {
        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                { name: "prepare", prepare: (): void => void 0 },
                { name: "finalize", finalize: (): void => void 0 },
            ],
        });

        expect(result.code).toBe("const a = 1;");
        expect(result.map?.version).toBe(3);
        expect(result.map?.mappings).toBe("AAAA,MAAM,IAAI");
    });

    it("composes a requested input map back to the upstream original", async (): Promise<void> => {
        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            sourceMap: {
                input: buildMap({
                    file: "upstream.ts",
                    mappings: "AASA",
                    names: [],
                    sources: ["upstream.ts"],
                    sourcesContent: ["const a = 1;"],
                }),
            },
        });

        // The upstream map resolves parse-input (0, 0) to original (9, 0);
        // composition carries that through the codegen map's src side.
        expect(result.map?.file).toBe("index.ts");
        expect(result.map?.sources).toEqual(["index.ts"]);
        expect(result.map?.mappings).toBe("AASA,MAAM,IAAI");
    });

    it("appends an inline map and drops the map end-to-end", async (): Promise<void> => {
        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            sourceMap: { output: "inline" },
        });

        expect(result.map).toBeNull();
        expect(result.code).toContain("//#" + " sourceMappingURL=");
    });

    it("runs concurrent compiles with mixed sync and async hooks", async (): Promise<void> => {
        const seen: Array<string> = [];

        const compiles: Array<Promise<CodeResult>> = [];
        for (let index: number = 0; index < 8; index += 1) {
            const marker: string = String(index);
            const isOdd: boolean = index % 2 === 1;
            compiles.push(
                compile("const a = 1;", {
                    cwd: "/repo",
                    file: "index.ts",
                    plugins: [
                        {
                            name: `mixed-${marker}`,
                            prepare: isOdd
                                ? async (): Promise<void> => void 0
                                : (): void => void 0,
                            transform: isOdd
                                ? (): void => {
                                      seen.push(marker);
                                  }
                                : async (): Promise<void> => {
                                      seen.push(marker);
                                  },
                        },
                    ],
                }),
            );
        }

        const results: Array<CodeResult> = await Promise.all(compiles);

        expect(results).toHaveLength(8);
        for (const result of results) expect(result.code).toBe("const a = 1;");
        expect(seen).toHaveLength(8);
        expect(seen).toEqual(
            expect.arrayContaining(["0", "1", "2", "3", "4", "5", "6", "7"]),
        );
    });
});
