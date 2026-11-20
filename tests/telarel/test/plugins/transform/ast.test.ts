import type {
    AstOptions,
    AstResult,
    CodeResult,
    FinalizeArgs,
    GenerateArgs,
    Language,
    ParseArgs,
    Plugin,
    PluginContext,
    PrepareArgs,
    Program,
    TransformArgs,
    TransformResult,
} from "telarel";

import type { ProgramFixture } from "#/functions/ast";

import { Ast, compile, parse, PluginError } from "telarel";
import { describe, expect, it } from "vitest";

import { renameRootIdentifier } from "#/functions/ast";

describe("transform AST", (): void => {
    it("exposes UTF-16 code-unit spans to the transform hook", async (): Promise<void> => {
        const source: string = "const µ = 1; const b = 2;";
        let start: number = -1;
        let end: number = -1;

        const plugin: Plugin = {
            name: "span-observer",
            transform: (_ctx: PluginContext, args: TransformArgs): void => {
                const statement = args.ast.program.body[1] as {
                    start: number;
                    end: number;
                };

                start = statement.start;
                end = statement.end;
            },
        };

        await compile(source, {
            cwd: "/repo",
            file: "index.js",
            plugins: [plugin],
        });

        expect(source.slice(start, end)).toBe("const b = 2;");
        expect(end - start).toBe("const b = 2;".length);
    });

    it("anchors UTF-16 spans to the parse-hook-returned source", async (): Promise<void> => {
        // A `parse` hook supplies the AST, so its spans anchor to the `code`
        // that the hook returns, not to the source of the compile call. The
        // test takes a real AST for the rewritten source from a first compile,
        // then returns it from the `parse` hook.
        const rewritten: string = "const µ = 1; const b = 2;";

        let snapshot: unknown;

        await compile(rewritten, {
            cwd: "/repo",
            file: "index.js",
            plugins: [
                {
                    name: "snapshotter",
                    transform: (
                        _ctx: PluginContext,
                        args: TransformArgs,
                    ): void => {
                        snapshot = structuredClone(args.ast.program);
                    },
                },
            ],
        });

        let start: number = -1;
        let end: number = -1;

        const plugin: Plugin = {
            name: "parse-supplier",
            parse: (_ctx: PluginContext, _args: ParseArgs): AstResult => ({
                ast: new Ast({
                    program: snapshot as AstOptions["program"],
                    source: rewritten,
                }),
            }),
            transform: (_ctx: PluginContext, args: TransformArgs): void => {
                const statement = args.ast.program.body[1] as unknown as {
                    start: number;
                    end: number;
                };

                start = statement.start;
                end = statement.end;
            },
        };

        await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.js",
            plugins: [plugin],
        });

        expect(rewritten.slice(start, end)).toBe("const b = 2;");
        expect(end - start).toBe("const b = 2;".length);
    });

    it("keeps comments when a transform mutates the AST", async (): Promise<void> => {
        const code: string = "// keep me\nconst a = 1;";

        const plugin: Plugin = {
            name: "comment-renamer",
            transform: (
                _ctx: PluginContext,
                args: TransformArgs,
            ): TransformResult => {
                args.ast.mutate((program: Program): void => {
                    renameRootIdentifier({
                        program: program as unknown as ProgramFixture,
                        name: "renamed",
                    });
                });

                return { ast: args.ast };
            },
        };

        const result: CodeResult = await compile(code, {
            cwd: "/repo",
            file: "index.js",
            plugins: [plugin],
        });

        expect(result.code).toContain("// keep me");
        expect(result.code).toContain("const renamed = 1;");
    });

    it("exposes the AST language to every AST-bearing hook", async (): Promise<void> => {
        const seen: Array<{ hook: string; language: Language | void }> = [];

        await compile("const a: number = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "language-observer",
                    prepare: (_ctx: PluginContext, args: PrepareArgs): void => {
                        seen.push({
                            hook: "prepare",
                            language: args.ast.language,
                        });
                    },
                    transform: (
                        _ctx: PluginContext,
                        args: TransformArgs,
                    ): TransformResult => {
                        seen.push({
                            hook: "transform",
                            language: args.ast.language,
                        });

                        return { ast: args.ast };
                    },
                    finalize: (
                        _ctx: PluginContext,
                        args: FinalizeArgs,
                    ): void => {
                        seen.push({
                            hook: "finalize",
                            language: args.ast.language,
                        });
                    },
                    generate: (
                        _ctx: PluginContext,
                        args: GenerateArgs,
                    ): null => {
                        seen.push({
                            hook: "generate",
                            language: args.ast.language,
                        });

                        return null;
                    },
                },
            ],
        });

        expect(seen).toEqual([
            { hook: "prepare", language: "ts" },
            { hook: "transform", language: "ts" },
            { hook: "finalize", language: "ts" },
            { hook: "generate", language: "ts" },
        ]);
    });

    it("exposes only the ast key on the transform hook payload", async (): Promise<void> => {
        const code: string = "// keep me\nconst a = 1;";
        let keys: Array<string> = [];
        let astKeys: Array<string> = [];
        let astSource: string = "";
        let astInstance: boolean = false;

        const plugin: Plugin = {
            name: "payload-shape",
            transform: (_ctx: PluginContext, args: TransformArgs): void => {
                keys = Object.keys(args);
                astKeys = Object.keys(args.ast);
                astSource = args.ast.source;
                astInstance = args.ast instanceof Ast;
            },
        };

        await compile(code, {
            cwd: "/repo",
            file: "index.js",
            plugins: [plugin],
        });

        expect(keys).toEqual(["ast"]);
        expect(astKeys).toEqual([]);
        expect(astInstance).toBe(true);
        expect(astSource).toBe(code);
        expect(keys).not.toContain("comments");
        expect(keys).not.toContain("commentsJson");
    });

    it("replaces the carried AST with the returned Ast", async (): Promise<void> => {
        const replacement: Ast = (
            await parse("const b = 2;", {
                cwd: "/repo",
                file: "index.js",
            })
        ).ast;

        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.js",
            plugins: [
                {
                    name: "replace",
                    transform: (): TransformResult => ({ ast: replacement }),
                },
            ],
        });

        expect(result.code).toBe("const b = 2;");
        expect(result.map?.sourcesContent).toEqual(["const b = 2;"]);
    });

    it("keeps a comment after non-ASCII code through a mutating transform", async (): Promise<void> => {
        const code: string = "const µ = 1;\n// keep me\nconst b = 2;";

        const plugin: Plugin = {
            name: "non-ascii-comment-renamer",
            transform: (
                _ctx: PluginContext,
                args: TransformArgs,
            ): TransformResult => {
                args.ast.mutate((program: Program): void => {
                    renameRootIdentifier({
                        program: program as unknown as ProgramFixture,
                        name: "renamed",
                    });
                });

                return { ast: args.ast };
            },
        };

        const result: CodeResult = await compile(code, {
            cwd: "/repo",
            file: "index.js",
            plugins: [plugin],
        });

        expect(result.code).toContain("// keep me");
        expect(result.code).toContain("const renamed = 1;");
    });

    it("keeps output byte-identical for a pass-through transform", async (): Promise<void> => {
        const code: string = "// keep me\nconst a = 1;";

        const plugin: Plugin = {
            name: "comment-passthrough",
            transform: (
                _ctx: PluginContext,
                args: TransformArgs,
            ): TransformResult => {
                return { ast: args.ast };
            },
        };

        const result: CodeResult = await compile(code, {
            cwd: "/repo",
            file: "index.js",
            plugins: [plugin],
        });

        expect(result.code).toBe(code);
    });

    it("applies a same-instance transform mutation through Ast.mutate", async (): Promise<void> => {
        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.js",
            plugins: [
                {
                    name: "mutate-and-return",
                    transform: (
                        _ctx: PluginContext,
                        args: TransformArgs,
                    ): TransformResult => {
                        args.ast.mutate((program: Program): void => {
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

    it("discards a transform mutation when the handler returns null", async (): Promise<void> => {
        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.js",
            plugins: [
                {
                    name: "mutate-and-null",
                    transform: (
                        _ctx: PluginContext,
                        args: TransformArgs,
                    ): null => {
                        args.ast.mutate((program: Program): void => {
                            renameRootIdentifier({
                                program: program as unknown as ProgramFixture,
                                name: "discarded",
                            });
                        });

                        return null;
                    },
                },
            ],
        });

        expect(result.code).toBe("const a = 1;");
    });

    it("keeps output unchanged when an async transform returns the same unmutated Ast", async (): Promise<void> => {
        const code: string = "const a = 1;";

        const result: CodeResult = await compile(code, {
            cwd: "/repo",
            file: "index.js",
            plugins: [
                {
                    name: "async-passthrough",
                    transform: async (
                        _ctx: PluginContext,
                        args: TransformArgs,
                    ): Promise<TransformResult> => {
                        await Promise.resolve();

                        return { ast: args.ast };
                    },
                },
            ],
        });

        expect(result.code).toBe(code);
        expect(result.map?.version).toBe(3);
    });

    it("reports the plugin error when a transform throws after Ast.mutate", async (): Promise<void> => {
        await expect(
            compile("const a = 1;", {
                cwd: "/repo",
                file: "index.js",
                plugins: [
                    {
                        name: "mutate-and-throw",
                        transform: (
                            _ctx: PluginContext,
                            args: TransformArgs,
                        ): never => {
                            args.ast.mutate((program: Program): void => {
                                renameRootIdentifier({
                                    program:
                                        program as unknown as ProgramFixture,
                                    name: "discarded",
                                });
                            });

                            throw new PluginError("mutate-boom");
                        },
                    },
                ],
            }),
        ).rejects.toThrow("mutate-boom");
    });
});
