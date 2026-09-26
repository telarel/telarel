import type { CompileResult, Plugin, PluginContext } from "telarel";

import { compile } from "telarel";
import { walk } from "telarel/walker";
import { describe, expect, it } from "vitest";

type ProgramFixture = {
    body: Array<{
        declarations: Array<{
            id: { type: string; name: string };
        }>;
    }>;
};

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

    it("skips ast read-back when the transform hook does not mutate", async (): Promise<void> => {
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
                    transform: (_ctx: PluginContext, args) => {
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

    it("shares metadata across plugins", async (): Promise<void> => {
        const seen: Array<unknown> = [];

        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                {
                    name: "writer",
                    pre: (ctx: PluginContext): void => {
                        ctx.metadata.set("marker", "from-writer");
                    },
                },
                {
                    name: "reader",
                    transform: (ctx: PluginContext) => {
                        seen.push(ctx.metadata.get("marker"));
                        // no mutation: pass-through
                    },
                },
                {
                    name: "verifier",
                    post: (ctx: PluginContext): void => {
                        seen.push(ctx.metadata.get("marker"));
                    },
                },
            ],
        });

        expect(result.code).toBe("const a = 1;");
        expect(seen).toEqual(["from-writer", "from-writer"]);
    });

    it("chains options hooks last-wins", async (): Promise<void> => {
        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                {
                    name: "opts",
                    options: (options): void => {
                        options.code = "const b = 2;";
                    },
                },
            ],
        });

        expect(result.code).toContain("const b = 2");
    });

    it("chains options hooks last-wins across a pass-through plugin", async (): Promise<void> => {
        // A no-op plugin between two mutating ones is a pass-through: the
        // first mutation is still visible to the second.
        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                {
                    name: "first",
                    options: (options): void => {
                        options.code = "const b = 2;";
                    },
                },
                {
                    name: "passthrough",
                    options: (): void => void 0,
                },
                {
                    name: "second",
                    options: (options): void => {
                        options.code = `${options.code}; const c = 3;`;
                    },
                },
            ],
        });

        expect(result.code).toContain("const b = 2");
        expect(result.code).toContain("const c = 3");
    });

    it("propagates in-place options mutations across plugins", async (): Promise<void> => {
        // Each plugin mutates the fields it wants; the second plugin
        // observes the first mutation and rewrites the file.
        const seen: Array<string> = [];

        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                {
                    name: "rewrite-code",
                    options: (options): void => {
                        options.code = "const b = 2;";
                    },
                },
                {
                    name: "rewrite-file",
                    options: (options): void => {
                        seen.push(options.code);
                        seen.push(options.cwd);

                        options.file = "renamed.ts";
                    },
                },
                {
                    name: "observe",
                    pre: (ctx: PluginContext): void => {
                        seen.push(ctx.file);
                        seen.push(ctx.code);
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
                    options: (options): void => {
                        seen.push(options.cwd);
                    },
                },
            ],
        });

        expect(seen).toEqual([expected]);
        expect(result.code).toBe("const a = 1;");
    });

    it("awaits an async options hook", async (): Promise<void> => {
        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                {
                    name: "async-options",
                    options: async (options): Promise<void> => {
                        await new Promise<void>((resolve): void => {
                            setTimeout(resolve, 0);
                        });
                        options.code = "const replaced = 2;";
                    },
                },
            ],
        });

        expect(result.code).toBe("const replaced = 2;");
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
            pre: (): void => {},
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
            pre: (): void => {},
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

    it("rejects on an error thrown from pre", async (): Promise<void> => {
        await expect(
            compile({
                cwd: "/repo",
                file: "index.ts",
                code: "const a = 1;",
                plugins: [
                    {
                        name: "boom-pre",
                        pre: (): never => {
                            throw new Error("pre-kaboom");
                        },
                    },
                ],
            }),
        ).rejects.toThrow("pre-kaboom");
    });

    it("rejects on an error thrown from post", async (): Promise<void> => {
        await expect(
            compile({
                cwd: "/repo",
                file: "index.ts",
                code: "const a = 1;",
                plugins: [
                    {
                        name: "boom-post",
                        post: (): never => {
                            throw new Error("post-kaboom");
                        },
                    },
                ],
            }),
        ).rejects.toThrow("post-kaboom");
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

    it("runs async pre, transform, and post hooks", async (): Promise<void> => {
        const seen: Array<string> = [];

        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                {
                    name: "async-all",
                    options: (options): void => {
                        seen.push("options");
                        options.code = "const b = 2;";
                    },
                    pre: async (ctx: PluginContext): Promise<void> => {
                        seen.push("pre");
                        ctx.metadata.set("marker", "from-async-pre");
                    },
                    transform: async (ctx: PluginContext) => {
                        seen.push("transform");
                        seen.push(String(ctx.metadata.get("marker")));
                    },
                    post: async (ctx: PluginContext): Promise<void> => {
                        seen.push("post");
                        seen.push(String(ctx.metadata.get("marker")));
                    },
                },
            ],
        });

        expect(result.code).toBe("const b = 2;");
        expect(seen).toEqual([
            "options",
            "pre",
            "transform",
            "from-async-pre",
            "post",
            "from-async-pre",
        ]);
    });

    it("orders pre, transform, post across plugins", async (): Promise<void> => {
        const seen: Array<string> = [];

        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                {
                    name: "first",
                    pre: (): void => {
                        seen.push("first.pre");
                    },
                    transform: (_ctx: PluginContext) => {
                        seen.push("first.transform");
                    },
                    post: (): void => {
                        seen.push("first.post");
                    },
                },
                {
                    name: "second",
                    pre: (): void => {
                        seen.push("second.pre");
                    },
                    transform: (_ctx: PluginContext) => {
                        seen.push("second.transform");
                    },
                    post: (): void => {
                        seen.push("second.post");
                    },
                },
            ],
        });

        expect(result.code).toBe("const a = 1;");
        expect(seen).toEqual([
            "first.pre",
            "second.pre",
            "first.transform",
            "second.transform",
            "first.post",
            "second.post",
        ]);
    });

    it("chains transforms ast to ast across plugins", async (): Promise<void> => {
        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                {
                    name: "rename-a",
                    transform: (_ctx: PluginContext, args) => {
                        const program: ProgramFixture =
                            args.ast as unknown as ProgramFixture;
                        const statement:
                            | ProgramFixture["body"][number]
                            | undefined = program.body[0];
                        const declaration:
                            | ProgramFixture["body"][number]["declarations"][number]
                            | undefined = statement?.declarations[0];
                        if (declaration?.id.type !== "Identifier") {
                            throw new Error(
                                "expected an identifier declaration",
                            );
                        }
                        declaration.id.name = "first";
                    },
                },
                {
                    name: "rename-b",
                    transform: (_ctx: PluginContext, args) => {
                        const program: ProgramFixture =
                            args.ast as unknown as ProgramFixture;
                        const statement:
                            | ProgramFixture["body"][number]
                            | undefined = program.body[0];
                        const declaration:
                            | ProgramFixture["body"][number]["declarations"][number]
                            | undefined = statement?.declarations[0];
                        if (declaration?.id.type !== "Identifier") {
                            throw new Error(
                                "expected an identifier declaration",
                            );
                        }
                        if (declaration.id.name !== "first") {
                            throw new Error(
                                "expected the first rename to be visible",
                            );
                        }
                        declaration.id.name = "second";
                    },
                },
            ],
        });

        expect(result.code).toBe("const second = 1;");
    });

    it("propagates metadata writes from transform to post", async (): Promise<void> => {
        const seen: Array<unknown> = [];

        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                {
                    name: "transform-writer",
                    transform: (ctx: PluginContext) => {
                        ctx.metadata.set("marker", "from-transform");
                    },
                },
                {
                    name: "post-reader",
                    post: (ctx: PluginContext): void => {
                        seen.push(ctx.metadata.get("marker"));
                    },
                },
            ],
        });

        expect(result.code).toBe("const a = 1;");
        expect(seen).toEqual(["from-transform"]);
    });

    it("isolates metadata across concurrent compiles", async (): Promise<void> => {
        const seen: Array<unknown> = [];

        const compileWith = async (name: string): Promise<CompileResult> =>
            await compile({
                cwd: "/repo",
                file: "index.ts",
                code: "const a = 1;",
                plugins: [
                    {
                        name: "writer",
                        pre: (ctx: PluginContext): void => {
                            ctx.metadata.set("marker", name);
                        },
                    },
                    {
                        name: "reader",
                        transform: (): void => void 0,
                    },
                    {
                        name: "verifier",
                        post: (ctx: PluginContext): void => {
                            seen.push(ctx.metadata.get("marker"));
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

    it("passes hook arguments with cwd, file, and code to each hook", async (): Promise<void> => {
        const seen: Array<{ cwd: string; file: string; code: string }> = [];

        const record = (
            ctx: PluginContext,
            args: { file: string; code: string },
        ): void => {
            seen.push({
                cwd: ctx.cwd,
                file: args.file,
                code: args.code,
            });
        };

        const first: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                {
                    name: "fidelity",
                    pre: (ctx: PluginContext, args): void => {
                        record(ctx, args);
                    },
                    transform: (ctx: PluginContext, args) => {
                        record(ctx, {
                            file: args.file,
                            code: ctx.code,
                        });
                    },
                    post: (ctx: PluginContext, args): void => {
                        record(ctx, args);
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
                    options: (options): void => {
                        options.code = "const b = 2;";
                    },
                },
                {
                    name: "fidelity",
                    pre: (ctx: PluginContext, args): void => {
                        record(ctx, args);
                    },
                    transform: (ctx: PluginContext, args) => {
                        record(ctx, {
                            file: args.file,
                            code: ctx.code,
                        });
                    },
                    post: (ctx: PluginContext, args): void => {
                        record(ctx, args);
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

    it("applies a nested ast mutation through json", async (): Promise<void> => {
        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: `const a = "old";`,
            plugins: [
                {
                    name: "nested-mutator",
                    transform: (_ctx: PluginContext, args) => {
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
                    },
                },
            ],
        });

        expect(result.code).toContain(`"new"`);
        expect(result.code).not.toContain(`"old"`);
        expect(result.map.mappings.length).toBeGreaterThan(0);
    });

    it("applies an in-place ast mutation when transform returns void", async (): Promise<void> => {
        // The wrapper re-stringifies the tree after the hook and compares it
        // with the pre-call string; the mutation flows back into Rust.
        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                {
                    name: "mutate-in-place",
                    transform: (_ctx: PluginContext, args): void => {
                        const program: ProgramFixture =
                            args.ast as unknown as ProgramFixture;

                        const statement:
                            | ProgramFixture["body"][number]
                            | undefined = program.body[0];

                        const declaration:
                            | ProgramFixture["body"][number]["declarations"][number]
                            | undefined = statement?.declarations[0];

                        if (declaration?.id.type !== "Identifier") {
                            throw new Error(
                                "expected an identifier declaration",
                            );
                        }

                        declaration.id.name = "mutated";
                    },
                },
            ],
        });

        expect(result.code).toBe("const mutated = 1;");
    });

    it("applies a whole-root reassignment of args.ast when transform returns void", async (): Promise<void> => {
        // The wrapper re-stringifies the args-bag property after the hook,
        // so a plugin can replace the whole tree (not just mutate it in
        // place) and the replacement must flow back into Rust.
        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                {
                    name: "root-reassign",
                    transform: (_ctx: PluginContext, args): void => {
                        const program: ProgramFixture = structuredClone(
                            args.ast,
                        ) as unknown as ProgramFixture;

                        const declaration:
                            | ProgramFixture["body"][number]["declarations"][number]
                            | undefined = program.body[0]?.declarations[0];

                        if (declaration?.id.type !== "Identifier") {
                            throw new Error(
                                "expected an identifier declaration",
                            );
                        }

                        declaration.id.name = "replaced";

                        args.ast = program as never;
                    },
                },
            ],
        });

        expect(result.code).toBe("const replaced = 1;");
    });

    it("silently ignores a returned value from transform", async (): Promise<void> => {
        // Intentional behavior change: the wrapper calls the hook and
        // silently ignores the return value (no `TypeError`), so a
        // replacement object returned without mutating is a pass-through
        // and the original code survives.
        const plugin: unknown = {
            name: "ignored-return",
            transform: (): unknown => ({
                ast: { type: "Program", body: [] },
            }),
        };

        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [plugin as Plugin],
        });

        expect(result.code).toBe("const a = 1;");
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
                    transform: (_ctx: PluginContext, args): void => {
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

    it("rejects with the late error after a prior plugin mutated the ast", async (): Promise<void> => {
        const build = async (): Promise<CompileResult> =>
            await compile({
                cwd: "/repo",
                file: "index.ts",
                code: "const a = 1;",
                plugins: [
                    {
                        name: "rename",
                        transform: (_ctx: PluginContext, args) => {
                            const program: ProgramFixture =
                                args.ast as unknown as ProgramFixture;
                            const statement:
                                | ProgramFixture["body"][number]
                                | undefined = program.body[0];
                            const declaration:
                                | ProgramFixture["body"][number]["declarations"][number]
                                | undefined = statement?.declarations[0];
                            if (declaration?.id.type !== "Identifier") {
                                throw new Error(
                                    "expected an identifier declaration",
                                );
                            }
                            declaration.id.name = "renamed";
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

    it("preserves non-string metadata values across hooks", async (): Promise<void> => {
        const seen: Array<unknown> = [];
        const seenObject: Array<unknown> = [];

        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                {
                    name: "writer",
                    pre: (ctx: PluginContext): void => {
                        ctx.metadata.set("count", 42);
                        ctx.metadata.set("obj", { nested: true });
                    },
                },
                {
                    name: "reader",
                    transform: (ctx: PluginContext) => {
                        seen.push(ctx.metadata.get("count"));
                        seenObject.push(ctx.metadata.get("obj"));
                    },
                },
            ],
        });

        expect(result.code).toBe("const a = 1;");
        expect(seen).toEqual([42]);
        expect(seenObject).toEqual([{ nested: true }]);
    });

    it("reuses the same plugin objects across repeated sequential compiles", async (): Promise<void> => {
        const writer: Plugin = {
            name: "loop-writer",
            pre: (ctx: PluginContext): void => {
                ctx.metadata.set("marker", "loop");
            },
        };
        const reader: Plugin = {
            name: "loop-reader",
            transform: (): void => void 0,
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
        }

        expect(results).toHaveLength(5);

        for (const result of results) {
            expect(result.code).toBe("const a = 1;");
            expect(result.map.mappings.length).toBeGreaterThan(0);
        }
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
                            pre: isOdd
                                ? async (ctx: PluginContext): Promise<void> => {
                                      ctx.metadata.set("id", marker);
                                  }
                                : (ctx: PluginContext): void => {
                                      ctx.metadata.set("id", marker);
                                  },
                            transform: isOdd
                                ? (ctx: PluginContext) => {
                                      seen.push(String(ctx.metadata.get("id")));
                                  }
                                : async (ctx: PluginContext) => {
                                      seen.push(String(ctx.metadata.get("id")));
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
