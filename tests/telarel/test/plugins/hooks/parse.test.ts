import type {
    AstOptions,
    AstResult,
    CodeResult,
    Language,
    ParseArgs,
    PluginContext,
    TransformArgs,
} from "telarel";

import { Ast, compile, parse } from "telarel";
import { describe, expect, it } from "vitest";

type CaptureProgramOptions = {
    code: string;
};

/**
 * Capture an AST with a pass-through compile for these hook tests.
 */
const captureProgram = async ({
    code,
}: CaptureProgramOptions): Promise<unknown> => {
    let snapshot: unknown;

    await compile(code, {
        cwd: "/repo",
        file: "index.ts",
        plugins: [
            {
                name: "snapshotter",
                transform: (_ctx: PluginContext, args: TransformArgs): void => {
                    snapshot = structuredClone(args.ast.program);
                },
            },
        ],
    });

    return snapshot;
};

type Program = AstOptions["program"];

describe("parse hooks", (): void => {
    it("parse: first non-null wins and later hooks are ignored", async (): Promise<void> => {
        const program: Program = (await captureProgram({
            code: "let custom = 1;",
        })) as Program;
        const seen: Array<string> = [];

        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "parser-first",
                    parse: (
                        _ctx: PluginContext,
                        _args: ParseArgs,
                    ): AstResult => {
                        seen.push("first");

                        return {
                            ast: new Ast({
                                program,
                                source: "let custom = 1;",
                            }),
                        };
                    },
                },
                {
                    name: "parser-second",
                    parse: (
                        _ctx: PluginContext,
                        _args: ParseArgs,
                    ): AstResult => {
                        seen.push("second");

                        return {
                            ast: new Ast({
                                program,
                                source: "let custom = 1;",
                            }),
                        };
                    },
                },
            ],
        });

        expect(seen).toEqual(["first"]);
        expect(result.code).toBe("let custom = 1;");
    });

    it("parse: parser options own the grammar over a returned Ast language", async (): Promise<void> => {
        const replacement: Ast = (
            await parse("const a = 1;", { file: "index.js" })
        ).ast;
        const seen: Array<Language | void> = [];

        const result: CodeResult = await compile("const b = 2;", {
            cwd: "/repo",
            file: "index.js",
            plugins: [
                {
                    name: "returned-language",
                    parse: (): AstResult => ({
                        ast: new Ast({
                            program: replacement.program as unknown as Program,
                            source: replacement.source,
                            language: "tsx",
                        }),
                    }),
                    transform: (
                        _ctx: PluginContext,
                        args: TransformArgs,
                    ): void => {
                        seen.push(args.ast.language);
                    },
                },
            ],
        });

        expect(seen).toEqual(["js"]);
        expect(result.code).toBe("const a = 1;");
    });

    it("parse: a null return defers to the next hook", async (): Promise<void> => {
        const program: Program = (await captureProgram({
            code: "let second = 2;",
        })) as Program;
        const seen: Array<string> = [];

        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "parser-first",
                    parse: (_ctx: PluginContext, _args: ParseArgs): null => {
                        seen.push("first");

                        return null;
                    },
                },
                {
                    name: "parser-second",
                    parse: (
                        _ctx: PluginContext,
                        _args: ParseArgs,
                    ): AstResult => {
                        seen.push("second");

                        return {
                            ast: new Ast({
                                program,
                                source: "let second = 2;",
                            }),
                        };
                    },
                },
            ],
        });

        expect(seen).toEqual(["first", "second"]);
        expect(result.code).toBe("let second = 2;");
    });

    it("parse: all hooks deferring falls back to the builtin parser", async (): Promise<void> => {
        const seen: Array<string> = [];

        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "defers",
                    parse: (_ctx: PluginContext, _args: ParseArgs): null => {
                        seen.push("defers");

                        return null;
                    },
                },
            ],
        });

        expect(seen).toEqual(["defers"]);
        expect(result.code).toBe("const a = 1;");
        expect(result.map?.version).toBe(3);
    });

    it("parse: surfaces a parse hook error unchanged", async (): Promise<void> => {
        await expect(
            compile("const a = 1;", {
                cwd: "/repo",
                file: "index.ts",
                plugins: [
                    {
                        name: "boom-parse",
                        parse: (): never => {
                            throw new Error("parse-kaboom");
                        },
                    },
                ],
            }),
        ).rejects.toThrow("parse-kaboom");
    });
});
