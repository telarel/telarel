import type {
    AstOptions,
    CodeResult,
    EndArgs,
    FinalizeArgs,
    GenerateArgs,
    PluginContext,
    PrepareArgs,
    TransformArgs,
    TransformResult,
} from "telarel";

import { Ast, compile } from "telarel";
import { describe, expect, it } from "vitest";

type Program = AstOptions["program"];

const clearBody = (ast: Ast): void => {
    ast.mutate((program: Program): void => {
        program.body = [];
    });
};

describe("AST hooks", (): void => {
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
});
