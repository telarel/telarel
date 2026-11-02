import type {
    AstResult,
    CodeResult,
    Comment,
    CommonPluginContext,
    DeepReadonly,
    EndArgs,
    FinalizeArgs,
    OptionsArgs,
    Plugin,
    PluginContext,
    PrepareArgs,
    Program,
    SourceMap,
    StartArgs,
    TransformArgs,
    TransformResult,
} from "telarel";
import type { MockInstance } from "vitest";

import type { ProgramFixture } from "#/functions/ast";

import { Ast, compile, parse, PluginError } from "telarel";
import { transform } from "telarel/plugins/transform";
import { describe, expect, it, vi } from "vitest";

import { readRootIdentifier, renameRootIdentifier } from "#/functions/ast";
import { buildMap } from "#/functions/sourcemap";

const mutateRootIdentifier = (options: { ast: Ast; name: string }): void => {
    options.ast.mutate((program: Program): void => {
        renameRootIdentifier({
            program: program as unknown as ProgramFixture,
            name: options.name,
        });
    });
};

const rootIdentifier = (ast: Ast): string =>
    readRootIdentifier(ast.program as unknown as ProgramFixture);

const withBatchSetting = async <Result>(
    value: string | undefined,
    body: () => Promise<Result>,
): Promise<Result> => {
    const previous: string | undefined = process.env.TELAREL_TRANSFORM_BATCH;

    if (value === void 0) {
        delete process.env.TELAREL_TRANSFORM_BATCH;
    } else {
        process.env.TELAREL_TRANSFORM_BATCH = value;
    }

    try {
        return await body();
    } finally {
        if (previous === void 0) {
            delete process.env.TELAREL_TRANSFORM_BATCH;
        } else {
            process.env.TELAREL_TRANSFORM_BATCH = previous;
        }
    }
};

const withBatchDisabled = async <Result>(
    body: () => Promise<Result>,
): Promise<Result> => await withBatchSetting("0", body);

type BatchTrace<Result> = {
    batches: Array<Array<string>>;
    result: Result;
};

const batchPluginNames = (
    calls: ReadonlyArray<ReadonlyArray<unknown>>,
): Array<Array<string>> =>
    calls
        .filter(
            (call: ReadonlyArray<unknown>): boolean =>
                call[0] === "[telarel] transform batch",
        )
        .map((call: ReadonlyArray<unknown>): Array<string> => {
            const payload: { plugins: Array<string> } = call[1] as {
                plugins: Array<string>;
            };

            return payload.plugins;
        });

const withBatchTrace = async <Result>(
    body: () => Promise<Result>,
    debugValue: string | null = "1",
): Promise<BatchTrace<Result>> => {
    const previous: string | undefined =
        process.env.TELAREL_TRANSFORM_BATCH_DEBUG;

    const spy: MockInstance<typeof console.debug> = vi
        .spyOn(console, "debug")
        .mockImplementation((): void => {});

    if (debugValue === null) {
        delete process.env.TELAREL_TRANSFORM_BATCH_DEBUG;
    } else {
        process.env.TELAREL_TRANSFORM_BATCH_DEBUG = debugValue;
    }

    try {
        const result: Result = await body();
        const batches: Array<Array<string>> = batchPluginNames(spy.mock.calls);

        return { batches, result };
    } finally {
        spy.mockRestore();

        if (previous === void 0) {
            delete process.env.TELAREL_TRANSFORM_BATCH_DEBUG;
        } else {
            process.env.TELAREL_TRANSFORM_BATCH_DEBUG = previous;
        }
    }
};

const buildMutator = (name: string): Plugin => ({
    name,
    transform: (_ctx: PluginContext, args: TransformArgs): TransformResult => {
        mutateRootIdentifier({
            ast: args.ast,
            name,
        });

        return { ast: args.ast };
    },
});

const thrown = (body: () => void): unknown => {
    try {
        body();
    } catch (error: unknown) {
        return error;
    }

    return void 0;
};

const captureRawDescriptor = async (name: string): Promise<Plugin> => {
    let captured: Plugin | undefined;

    const plugin: Plugin = {
        name,
        options: (_ctx: CommonPluginContext, args: OptionsArgs): void => {
            for (const candidate of args.options.plugins ?? []) {
                const candidateName: string | undefined = (
                    candidate as unknown as { name?: string }
                ).name;

                if (candidateName === name) {
                    captured = candidate as unknown as Plugin;
                }
            }
        },
        transform: (
            _ctx: PluginContext,
            args: TransformArgs,
        ): TransformResult => {
            mutateRootIdentifier({
                ast: args.ast,
                name,
            });

            return { ast: args.ast };
        },
    };

    await withBatchDisabled(
        async (): Promise<CodeResult> =>
            await compile("const a = 1;", {
                cwd: "/repo",
                file: "index.js",
                plugins: [plugin],
            }),
    );

    if (captured === void 0) {
        throw new Error("expected a raw transform descriptor");
    }

    return captured;
};

const buildEquivalencePlugins = (): Array<Plugin> => [
    {
        name: "first",
        transform: (
            _ctx: PluginContext,
            args: TransformArgs,
        ): TransformResult => {
            mutateRootIdentifier({
                ast: args.ast,
                name: "first",
            });

            return { ast: args.ast };
        },
    },
    {
        name: "passthrough",
        transform: (
            _ctx: PluginContext,
            args: TransformArgs,
        ): TransformResult => ({ ast: args.ast }),
    },
    {
        name: "second",
        transform: (
            _ctx: PluginContext,
            args: TransformArgs,
        ): TransformResult => {
            mutateRootIdentifier({
                ast: args.ast,
                name: "second",
            });

            return { ast: args.ast };
        },
    },
];

describe("transform batch", (): void => {
    it("runs adjacent transforms in order and carries the returned AST", async (): Promise<void> => {
        const seen: Array<string> = [];

        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.js",
            plugins: [
                {
                    name: "first",
                    transform: (
                        _ctx: PluginContext,
                        args: TransformArgs,
                    ): TransformResult => {
                        seen.push(`first-before:${rootIdentifier(args.ast)}`);

                        mutateRootIdentifier({
                            ast: args.ast,
                            name: "first",
                        });

                        return { ast: args.ast };
                    },
                },
                {
                    name: "second",
                    transform: (
                        _ctx: PluginContext,
                        args: TransformArgs,
                    ): TransformResult => {
                        seen.push(`second-before:${rootIdentifier(args.ast)}`);

                        mutateRootIdentifier({
                            ast: args.ast,
                            name: "second",
                        });

                        return { ast: args.ast };
                    },
                },
            ],
        });

        expect(seen).toEqual(["first-before:a", "second-before:first"]);
        expect(result.code).toBe("const second = 1;");
    });

    it("runs two adjacent transforms in one batch call", async (): Promise<void> => {
        const { batches, result } = await withBatchTrace(
            async (): Promise<CodeResult> =>
                await compile("const a = 1;", {
                    cwd: "/repo",
                    file: "index.js",
                    plugins: [buildMutator("js1"), buildMutator("js2")],
                }),
        );

        expect(batches).toEqual([["js1", "js2"]]);
        expect(result.code).toBe("const js2 = 1;");
    });

    it("runs a batch when TELAREL_TRANSFORM_BATCH is 1", async (): Promise<void> => {
        const { batches, result } = await withBatchTrace(
            async (): Promise<CodeResult> =>
                await withBatchSetting(
                    "1",
                    async (): Promise<CodeResult> =>
                        await compile("const a = 1;", {
                            cwd: "/repo",
                            file: "index.js",
                            plugins: [buildMutator("js1"), buildMutator("js2")],
                        }),
                ),
        );

        expect(batches).toEqual([["js1", "js2"]]);
        expect(result.code).toBe("const js2 = 1;");
    });

    it("splits batch calls around a builtin transform", async (): Promise<void> => {
        const { batches, result } = await withBatchTrace(
            async (): Promise<CodeResult> =>
                await compile("const a = 1;", {
                    cwd: "/repo",
                    file: "index.js",
                    plugins: [
                        buildMutator("js1"),
                        buildMutator("js2"),
                        transform(),
                        buildMutator("js3"),
                        buildMutator("js4"),
                    ],
                }),
        );

        expect(batches).toEqual([
            ["js1", "js2"],
            ["js3", "js4"],
        ]);
        expect(result.code).toBe("const js4 = 1;");
    });

    it("records every plugin name in a three-plugin batch", async (): Promise<void> => {
        const { batches, result } = await withBatchTrace(
            async (): Promise<CodeResult> =>
                await compile("const a = 1;", {
                    cwd: "/repo",
                    file: "index.js",
                    plugins: [
                        buildMutator("js1"),
                        buildMutator("js2"),
                        buildMutator("js3"),
                    ],
                }),
        );

        expect(batches).toEqual([["js1", "js2", "js3"]]);
        expect(result.code).toBe("const js3 = 1;");
    });

    it("keeps the batch trace silent unless enabled", async (): Promise<void> => {
        const debugValues: Array<string | null> = [null, "0", "off"];

        for (const debugValue of debugValues) {
            const { batches, result } = await withBatchTrace(
                async (): Promise<CodeResult> =>
                    await compile("const a = 1;", {
                        cwd: "/repo",
                        file: "index.js",
                        plugins: [buildMutator("js1"), buildMutator("js2")],
                    }),
                debugValue,
            );

            expect(batches).toEqual([]);
            expect(result.code).toBe("const js2 = 1;");
        }
    });

    it("discards a mutate-and-null member before the next member", async (): Promise<void> => {
        const seen: Array<string> = [];

        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.js",
            plugins: [
                {
                    name: "discard",
                    transform: (
                        _ctx: PluginContext,
                        args: TransformArgs,
                    ): null => {
                        mutateRootIdentifier({
                            ast: args.ast,
                            name: "discarded",
                        });

                        return null;
                    },
                },
                {
                    name: "keep",
                    transform: (
                        _ctx: PluginContext,
                        args: TransformArgs,
                    ): TransformResult => {
                        seen.push(rootIdentifier(args.ast));

                        mutateRootIdentifier({
                            ast: args.ast,
                            name: "kept",
                        });

                        return { ast: args.ast };
                    },
                },
            ],
        });

        expect(seen).toEqual(["a"]);
        expect(result.code).toBe("const kept = 1;");
    });

    it("carries a mutate-and-return member into the next member", async (): Promise<void> => {
        const seen: Array<string> = [];

        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.js",
            plugins: [
                {
                    name: "carry",
                    transform: (
                        _ctx: PluginContext,
                        args: TransformArgs,
                    ): TransformResult => {
                        mutateRootIdentifier({
                            ast: args.ast,
                            name: "carried",
                        });

                        return { ast: args.ast };
                    },
                },
                {
                    name: "observe",
                    transform: (
                        _ctx: PluginContext,
                        args: TransformArgs,
                    ): TransformResult => {
                        seen.push(rootIdentifier(args.ast));

                        mutateRootIdentifier({
                            ast: args.ast,
                            name: "final",
                        });

                        return { ast: args.ast };
                    },
                },
            ],
        });

        expect(seen).toEqual(["carried"]);
        expect(result.code).toBe("const final = 1;");
    });

    it("carries a fresh AST with source, map, and language", async (): Promise<void> => {
        const source: string = "const fresh = 1;";

        const parsed: AstResult = await parse(source, {
            cwd: "/repo",
            file: "fresh.js",
        });

        const map: SourceMap = buildMap({
            sources: ["fresh.js"],
            mappings: "AAAA",
        });

        const replacement: Ast = new Ast({
            program: parsed.ast.program as Program,
            source,
            language: "js",
            map,
        });

        const seen: Array<string> = [];

        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.js",
            plugins: [
                {
                    name: "replace",
                    transform: (): TransformResult => ({ ast: replacement }),
                },
                {
                    name: "observe",
                    transform: (
                        _ctx: PluginContext,
                        args: TransformArgs,
                    ): TransformResult => {
                        seen.push(
                            [
                                args.ast.source,
                                args.ast.language ?? "none",
                                args.ast.map?.sources[0] ?? "none",
                            ].join("|"),
                        );

                        return { ast: args.ast };
                    },
                },
            ],
        });

        expect(seen).toEqual(["const fresh = 1;|js|fresh.js"]);
        expect(result.code).toBe("const fresh = 1;");
    });

    it("restores the full checkpoint after a mutating null member", async (): Promise<void> => {
        const source: string = "const fresh = 1;";

        const parsed: AstResult = await parse(source, {
            cwd: "/repo",
            file: "fresh.js",
        });

        const map: SourceMap = buildMap({
            sources: ["fresh.js"],
            mappings: "AAAA",
        });

        const replacement: Ast = new Ast({
            program: parsed.ast.program as Program,
            source,
            language: "js",
            map,
        });

        const seen: Array<string> = [];

        const { batches, result } = await withBatchTrace(
            async (): Promise<CodeResult> =>
                await compile("const a: number = 1;", {
                    cwd: "/repo",
                    file: "index.ts",
                    plugins: [
                        {
                            name: "replace",
                            transform: (): TransformResult => ({
                                ast: replacement,
                            }),
                        },
                        {
                            name: "mutate-null",
                            transform: (
                                _ctx: PluginContext,
                                args: TransformArgs,
                            ): null => {
                                mutateRootIdentifier({
                                    ast: args.ast,
                                    name: "discarded",
                                });

                                return null;
                            },
                        },
                        {
                            name: "observe",
                            transform: (
                                _ctx: PluginContext,
                                args: TransformArgs,
                            ): TransformResult => {
                                seen.push(
                                    [
                                        args.ast.source,
                                        args.ast.language ?? "none",
                                        args.ast.map?.sources[0] ?? "none",
                                        rootIdentifier(args.ast),
                                    ].join("|"),
                                );

                                return { ast: args.ast };
                            },
                        },
                    ],
                }),
        );

        expect(batches).toEqual([["replace", "mutate-null", "observe"]]);
        expect(seen).toEqual(["const fresh = 1;|js|fresh.js|fresh"]);
        expect(result.code).toBe("const fresh = 1;");
    });

    it("falls back to the input language for a fresh AST without one", async (): Promise<void> => {
        const source: string = "const fresh = 1;";

        const parsed: AstResult = await parse(source, {
            cwd: "/repo",
            file: "fresh.js",
        });

        const replacement: Ast = new Ast({
            program: parsed.ast.program as Program,
            source,
        });

        const seen: Array<string> = [];

        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "replace",
                    transform: (): TransformResult => ({ ast: replacement }),
                },
                {
                    name: "observe",
                    transform: (
                        _ctx: PluginContext,
                        args: TransformArgs,
                    ): TransformResult => {
                        seen.push(args.ast.language ?? "none");

                        return { ast: args.ast };
                    },
                },
            ],
        });

        expect(seen).toEqual(["ts"]);
        expect(result.code).toBe("const fresh = 1;");
    });

    it("propagates an error from a later member after an earlier change", async (): Promise<void> => {
        await expect(
            compile("const a = 1;", {
                cwd: "/repo",
                file: "index.js",
                plugins: [
                    {
                        name: "change",
                        transform: (
                            _ctx: PluginContext,
                            args: TransformArgs,
                        ): TransformResult => {
                            mutateRootIdentifier({
                                ast: args.ast,
                                name: "changed",
                            });

                            return { ast: args.ast };
                        },
                    },
                    {
                        name: "fail",
                        transform: (): never => {
                            throw new PluginError("second-boom");
                        },
                    },
                ],
            }),
        ).rejects.toThrow("second-boom");
    });

    it("breaks a run at a builtin transform and still works", async (): Promise<void> => {
        const seen: Array<string> = [];

        const result: CodeResult = await compile("const a: number = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "first",
                    transform: (
                        _ctx: PluginContext,
                        args: TransformArgs,
                    ): TransformResult => {
                        mutateRootIdentifier({
                            ast: args.ast,
                            name: "first",
                        });

                        return { ast: args.ast };
                    },
                },
                transform(),
                {
                    name: "second",
                    transform: (
                        _ctx: PluginContext,
                        args: TransformArgs,
                    ): TransformResult => {
                        seen.push(rootIdentifier(args.ast));

                        mutateRootIdentifier({
                            ast: args.ast,
                            name: "second",
                        });

                        return { ast: args.ast };
                    },
                },
            ],
        });

        expect(seen).toEqual(["first"]);
        expect(result.code).toBe("const second = 1;");
    });

    it("keeps prepare and finalize as per-plugin observers", async (): Promise<void> => {
        const seenPrepare: Array<string> = [];
        const seenFinalize: Array<string> = [];

        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.js",
            plugins: [
                {
                    name: "one",
                    prepare: (_ctx: PluginContext, args: PrepareArgs): void => {
                        seenPrepare.push(rootIdentifier(args.ast));
                    },
                    transform: (
                        _ctx: PluginContext,
                        args: TransformArgs,
                    ): TransformResult => {
                        mutateRootIdentifier({
                            ast: args.ast,
                            name: "one",
                        });

                        return { ast: args.ast };
                    },
                    finalize: (
                        _ctx: PluginContext,
                        args: FinalizeArgs,
                    ): void => {
                        seenFinalize.push(rootIdentifier(args.ast));
                    },
                },
                {
                    name: "two",
                    transform: (
                        _ctx: PluginContext,
                        args: TransformArgs,
                    ): TransformResult => {
                        mutateRootIdentifier({
                            ast: args.ast,
                            name: "two",
                        });

                        return { ast: args.ast };
                    },
                },
            ],
        });

        expect(seenPrepare).toEqual(["a"]);
        expect(seenFinalize).toEqual(["two"]);
        expect(result.code).toBe("const two = 1;");
    });

    it("reports the original plugin names to start and end", async (): Promise<void> => {
        const started: Array<Array<string>> = [];
        const ended: Array<number> = [];

        await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.js",
            plugins: [
                {
                    name: "one",
                    start: (_ctx: PluginContext, args: StartArgs): void => {
                        started.push([...args.options.plugins]);
                    },
                    end: (_ctx: PluginContext, _args: EndArgs): void => {
                        ended.push(1);
                    },
                    transform: (
                        _ctx: PluginContext,
                        args: TransformArgs,
                    ): TransformResult => {
                        mutateRootIdentifier({
                            ast: args.ast,
                            name: "one",
                        });

                        return { ast: args.ast };
                    },
                },
                {
                    name: "two",
                    transform: (
                        _ctx: PluginContext,
                        args: TransformArgs,
                    ): TransformResult => {
                        mutateRootIdentifier({
                            ast: args.ast,
                            name: "two",
                        });

                        return { ast: args.ast };
                    },
                },
                transform(),
            ],
        });

        expect(started).toEqual([["one", "two", "builtin:transform"]]);
        expect(ended).toEqual([1]);
    });

    it("runs async transform members in order", async (): Promise<void> => {
        const seen: Array<string> = [];

        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.js",
            plugins: [
                {
                    name: "first",
                    transform: async (
                        _ctx: PluginContext,
                        args: TransformArgs,
                    ): Promise<TransformResult> => {
                        await Promise.resolve();

                        mutateRootIdentifier({
                            ast: args.ast,
                            name: "asyncFirst",
                        });

                        return { ast: args.ast };
                    },
                },
                {
                    name: "second",
                    transform: async (
                        _ctx: PluginContext,
                        args: TransformArgs,
                    ): Promise<TransformResult> => {
                        await Promise.resolve();

                        seen.push(rootIdentifier(args.ast));

                        mutateRootIdentifier({
                            ast: args.ast,
                            name: "asyncSecond",
                        });

                        return { ast: args.ast };
                    },
                },
            ],
        });

        expect(seen).toEqual(["asyncFirst"]);
        expect(result.code).toBe("const asyncSecond = 1;");
    });

    it("still applies every transform with batching disabled", async (): Promise<void> => {
        const { batches, result } = await withBatchTrace(
            async (): Promise<CodeResult> =>
                await withBatchDisabled(
                    async (): Promise<CodeResult> =>
                        await compile("const a = 1;", {
                            cwd: "/repo",
                            file: "index.js",
                            plugins: [
                                buildMutator("disabledFirst"),
                                buildMutator("disabledSecond"),
                            ],
                        }),
                ),
        );

        expect(batches).toEqual([]);
        expect(result.code).toBe("const disabledSecond = 1;");
    });

    it("runs a mixed list per plugin with batching disabled", async (): Promise<void> => {
        const { batches, result } = await withBatchTrace(
            async (): Promise<CodeResult> =>
                await withBatchDisabled(
                    async (): Promise<CodeResult> =>
                        await compile("const a: number = 1;", {
                            cwd: "/repo",
                            file: "index.ts",
                            plugins: [
                                buildMutator("js1"),
                                buildMutator("js2"),
                                transform(),
                                buildMutator("js3"),
                                buildMutator("js4"),
                            ],
                        }),
                ),
        );

        expect(batches).toEqual([]);
        expect(result.code).toBe("const js4 = 1;");
    });

    it("batches around a raw descriptor without a batch runner", async (): Promise<void> => {
        const raw: Plugin = await captureRawDescriptor("raw");
        const seen: Array<string> = [];

        const { batches, result } = await withBatchTrace(
            async (): Promise<CodeResult> =>
                await compile("const a = 1;", {
                    cwd: "/repo",
                    file: "index.js",
                    plugins: [
                        buildMutator("js1"),
                        buildMutator("js2"),
                        raw,
                        {
                            name: "js3",
                            transform: (
                                _ctx: PluginContext,
                                args: TransformArgs,
                            ): TransformResult => {
                                seen.push(rootIdentifier(args.ast));

                                mutateRootIdentifier({
                                    ast: args.ast,
                                    name: "js3",
                                });

                                return { ast: args.ast };
                            },
                        },
                        buildMutator("js4"),
                    ],
                }),
        );

        expect(batches).toEqual([
            ["js1", "js2"],
            ["js3", "js4"],
        ]);
        expect(seen).toEqual(["raw"]);
        expect(result.code).toBe("const js4 = 1;");
    });

    it("keeps code and source map identical with batching enabled and disabled", async (): Promise<void> => {
        const code: string = "const µ = 1;\n// keep me\nconst b = 2;";

        const enabled: CodeResult = await compile(code, {
            cwd: "/repo",
            file: "index.js",
            plugins: buildEquivalencePlugins(),
        });

        const disabled: BatchTrace<CodeResult> = await withBatchTrace(
            async (): Promise<CodeResult> =>
                await withBatchDisabled(
                    async (): Promise<CodeResult> =>
                        await compile(code, {
                            cwd: "/repo",
                            file: "index.js",
                            plugins: buildEquivalencePlugins(),
                        }),
                ),
        );

        expect(disabled.batches).toEqual([]);
        expect(disabled.result.code).toBe(enabled.code);
        expect(disabled.result.map).toEqual(enabled.map);
    });

    it("preserves and re-anchors comments through adjacent pass-through transforms", async (): Promise<void> => {
        const code: string = "const a = 1; // tail\n";
        const seen: Array<string> = [];

        const { batches, result } = await withBatchTrace(
            async (): Promise<CodeResult> =>
                await compile(code, {
                    cwd: "/repo",
                    file: "index.js",
                    plugins: [
                        {
                            name: "first",
                            transform: (
                                _ctx: PluginContext,
                                args: TransformArgs,
                            ): TransformResult => ({ ast: args.ast }),
                        },
                        {
                            name: "second",
                            transform: (
                                _ctx: PluginContext,
                                args: TransformArgs,
                            ): TransformResult => ({ ast: args.ast }),
                        },
                        transform(),
                        {
                            name: "observer",
                            transform: (
                                _ctx: PluginContext,
                                args: TransformArgs,
                            ): TransformResult => {
                                const comments: ReadonlyArray<
                                    DeepReadonly<Comment>
                                > = args.ast.program.comments ?? [];

                                seen.push(
                                    comments
                                        .map((comment): string =>
                                            [
                                                comment.value,
                                                comment.position ?? "none",
                                            ].join("|"),
                                        )
                                        .join(","),
                                );

                                return { ast: args.ast };
                            },
                        },
                    ],
                }),
        );

        expect(batches).toEqual([["first", "second"]]);
        expect(seen).toEqual([" tail|leading"]);
        expect(result.code).toBe("const a = 1;\n// tail");
    });

    it("restores the checkpoint after an accepted change", async (): Promise<void> => {
        const seen: Array<string> = [];

        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.js",
            plugins: [
                {
                    name: "accept",
                    transform: (
                        _ctx: PluginContext,
                        args: TransformArgs,
                    ): TransformResult => {
                        mutateRootIdentifier({
                            ast: args.ast,
                            name: "accepted",
                        });

                        return { ast: args.ast };
                    },
                },
                {
                    name: "discard",
                    transform: (
                        _ctx: PluginContext,
                        args: TransformArgs,
                    ): null => {
                        mutateRootIdentifier({
                            ast: args.ast,
                            name: "discarded",
                        });

                        return null;
                    },
                },
                {
                    name: "observe",
                    transform: (
                        _ctx: PluginContext,
                        args: TransformArgs,
                    ): TransformResult => {
                        seen.push(rootIdentifier(args.ast));

                        mutateRootIdentifier({
                            ast: args.ast,
                            name: "final",
                        });

                        return { ast: args.ast };
                    },
                },
            ],
        });

        expect(seen).toEqual(["accepted"]);
        expect(result.code).toBe("const final = 1;");
    });
});

describe("root identifier helpers", (): void => {
    const program: ProgramFixture = {
        body: [{}],
    } as unknown as ProgramFixture;

    it("readRootIdentifier throws the intended error for a non-declaration statement", (): void => {
        const error: unknown = thrown((): void => {
            readRootIdentifier(program);
        });

        expect(error).toBeInstanceOf(Error);
        expect(error).not.toBeInstanceOf(TypeError);
        expect((error as Error).message).toBe(
            "expected an identifier declaration",
        );
    });

    it("renameRootIdentifier throws the intended error for a non-declaration statement", (): void => {
        const error: unknown = thrown((): void => {
            renameRootIdentifier({ program, name: "renamed" });
        });

        expect(error).toBeInstanceOf(Error);
        expect(error).not.toBeInstanceOf(TypeError);
        expect((error as Error).message).toBe(
            "expected an identifier declaration",
        );
    });
});
