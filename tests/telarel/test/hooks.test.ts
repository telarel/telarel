import type {
    AstOptions,
    AstResult,
    CodeResult,
    EndArgs,
    FinalizeArgs,
    GenerateArgs,
    Generator,
    ParseArgs,
    Parser,
    PluginContext,
    PrepareArgs,
    SourceMap,
    TransformArgs,
    TransformResult,
} from "telarel";

import {
    Ast,
    CompileError,
    compile,
    createGenerator,
    createParser,
} from "telarel";
import { describe, expect, it } from "vitest";

type CaptureProgramOptions = {
    code: string;
};

/**
 * JS tests have no parser; capture an AST via a pass-through compile.
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

const clearBody = (ast: Ast): void => {
    ast.mutate((program: Program): void => {
        program.body = [];
    });
};

describe("hooks", (): void => {
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

    it("generate: first non-null wins and later hooks are ignored", async (): Promise<void> => {
        const seen: Array<string> = [];

        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "gen-first",
                    generate: (): CodeResult => {
                        seen.push("first");

                        return { code: "first();", map: null };
                    },
                },
                {
                    name: "gen-second",
                    generate: (): CodeResult => {
                        seen.push("second");

                        return { code: "second();" };
                    },
                },
            ],
        });

        expect(seen).toEqual(["first"]);
        expect(result.code).toBe("first();");
        expect(result.map).toBeNull();
    });

    it("generate: a null return defers to the builtin generator", async (): Promise<void> => {
        const seen: Array<string> = [];

        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "gen-defer",
                    generate: (): null => {
                        seen.push("defer");

                        return null;
                    },
                },
            ],
        });

        expect(seen).toEqual(["defer"]);
        expect(result.code).toBe("const a = 1;");
        expect(result.map?.version).toBe(3);
    });

    it("generate: carries a returned map through", async (): Promise<void> => {
        const map: SourceMap = {
            version: 3,
            sources: ["index.ts"],
            sourcesContent: ["const a = 1;"],
            names: [],
            mappings: "AAAA",
        };

        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "gen-map",
                    generate: (): CodeResult => ({
                        code: "custom();",
                        map,
                    }),
                },
            ],
        });

        expect(result.code).toBe("custom();");
        expect(result.map?.mappings).toBe("AAAA");
        expect(result.map?.sources).toEqual(["index.ts"]);
    });

    it("generate: surfaces a generate hook error unchanged", async (): Promise<void> => {
        await expect(
            compile("const a = 1;", {
                cwd: "/repo",
                file: "index.ts",
                plugins: [
                    {
                        name: "boom-generate",
                        generate: (): never => {
                            throw new Error("generate-kaboom");
                        },
                    },
                ],
            }),
        ).rejects.toThrow("generate-kaboom");
    });

    it("prepare and finalize observe only the ast key", async (): Promise<void> => {
        const prepareKeys: Array<Array<string>> = [];
        const finalizeKeys: Array<Array<string>> = [];

        await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "prepare-observer",
                    prepare: (_ctx: PluginContext, args: PrepareArgs): void => {
                        prepareKeys.push(Object.keys(args));
                    },
                },
                {
                    name: "finalize-observer",
                    finalize: (
                        _ctx: PluginContext,
                        args: FinalizeArgs,
                    ): void => {
                        finalizeKeys.push(Object.keys(args));
                    },
                },
            ],
        });

        expect(prepareKeys).toEqual([["ast"]]);
        expect(finalizeKeys).toEqual([["ast"]]);
    });

    it("passes Ast to every AST hook", async (): Promise<void> => {
        let prepareAst: unknown;
        let transformAst: unknown;
        let finalizeAst: unknown;
        let generateAst: unknown;

        await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "ast-classes",
                    prepare: (_ctx: PluginContext, args: PrepareArgs): void => {
                        prepareAst = args.ast;
                    },
                    transform: (
                        _ctx: PluginContext,
                        args: TransformArgs,
                    ): TransformResult => {
                        transformAst = args.ast;

                        return { ast: args.ast };
                    },
                    finalize: (
                        _ctx: PluginContext,
                        args: FinalizeArgs,
                    ): void => {
                        finalizeAst = args.ast;
                    },
                    generate: (
                        _ctx: PluginContext,
                        args: GenerateArgs,
                    ): null => {
                        generateAst = args.ast;

                        return null;
                    },
                },
            ],
        });

        expect(prepareAst).toBeInstanceOf(Ast);
        expect(prepareAst).toHaveProperty("mutate");
        expect(finalizeAst).toBeInstanceOf(Ast);
        expect(finalizeAst).toHaveProperty("mutate");
        expect(generateAst).toBeInstanceOf(Ast);
        expect(generateAst).toHaveProperty("mutate");
        expect(transformAst).toBeInstanceOf(Ast);
        expect(transformAst).toHaveProperty("mutate");
    });

    it("does not apply mutations made by prepare, finalize, or generate hooks", async (): Promise<void> => {
        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "observer-mutator",
                    prepare: (_ctx: PluginContext, args: PrepareArgs): void => {
                        clearBody(args.ast);
                    },
                    finalize: (
                        _ctx: PluginContext,
                        args: FinalizeArgs,
                    ): void => {
                        clearBody(args.ast);
                    },
                    generate: (
                        _ctx: PluginContext,
                        args: GenerateArgs,
                    ): null => {
                        clearBody(args.ast);

                        return null;
                    },
                },
            ],
        });

        expect(result.code).toBe("const a = 1;");
    });

    it("end receives only the error key on the success path", async (): Promise<void> => {
        const seen: Array<{ keys: Array<string>; error: string | null }> = [];

        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "end-observer",
                    end: (_ctx: PluginContext, args: EndArgs): void => {
                        seen.push({
                            keys: Object.keys(args),
                            error: args.error,
                        });
                    },
                },
            ],
        });

        expect(result.code).toBe("const a = 1;");
        expect(seen).toEqual([{ keys: ["error"], error: null }]);
    });

    it("end receives the first stage error on failure", async (): Promise<void> => {
        const seen: Array<string> = [];

        await expect(
            compile("const a = 1;", {
                cwd: "/repo",
                file: "index.ts",
                plugins: [
                    {
                        name: "end-observer",
                        end: (_ctx: PluginContext, args: EndArgs): void => {
                            seen.push(args.error ?? "");
                        },
                    },
                    {
                        name: "boom",
                        transform: (): never => {
                            throw new Error("kaboom");
                        },
                    },
                ],
            }),
        ).rejects.toThrow("kaboom");

        expect(seen).toHaveLength(1);
        expect(seen[0]).toContain("kaboom");
    });

    it("drives a custom language through parse and generate hooks only", async (): Promise<void> => {
        // The custom source is invalid JS, so only plugin parse and generate
        // hooks can avoid both builtins.
        const custom: string = "greet world!";
        const program: Program = (await captureProgram({
            code: "const fromCustom = 1;",
        })) as Program;

        await expect(
            compile(custom, { cwd: "/repo", file: "index.ts" }),
        ).rejects.toSatisfy(
            (error: unknown): boolean =>
                error instanceof CompileError &&
                error.render().includes("index.ts"),
        );

        const result: CodeResult = await compile(custom, {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "custom-language",
                    parse: (): AstResult => ({
                        ast: new Ast({ program, source: custom }),
                    }),
                    generate: (
                        _ctx: PluginContext,
                        args: GenerateArgs,
                    ): CodeResult => {
                        // The generate hook sees the AST the parse hook put in
                        // place, not one built from `custom`.
                        expect(args.ast.source).toBe(custom);

                        return { code: "// custom output", map: null };
                    },
                },
            ],
        });

        expect(result.code).toBe("// custom output");
        expect(result.map).toBeNull();
    });

    it("passes createParser/createGenerator results through hooks verbatim", async (): Promise<void> => {
        const seen: Array<string> = [];

        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "builtin-verbatim",
                    parse: async (
                        _ctx: PluginContext,
                        args: ParseArgs,
                    ): Promise<AstResult> => {
                        const parser: Parser = createParser(
                            args.code,
                            args.options,
                        );

                        seen.push("parse");

                        return await parser.parse();
                    },
                    generate: async (
                        _ctx: PluginContext,
                        args: GenerateArgs,
                    ): Promise<CodeResult> => {
                        const generator: Generator = createGenerator(
                            args.ast,
                            args.options,
                        );

                        seen.push("generate");

                        return await generator.generate();
                    },
                },
            ],
        });

        expect(seen).toEqual(["parse", "generate"]);
        expect(result.code).toBe("const a = 1;");
        expect(result.map?.version).toBe(3);
    });
});
