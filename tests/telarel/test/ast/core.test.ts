import type {
    AstOptions,
    AstResult,
    CodeResult,
    PluginContext,
    SourceMap,
    TransformArgs,
    TransformResult,
} from "telarel";
import type { OxcProgram, Program as AstProgram } from "telarel/ast";

import type { ProgramFixture } from "#/functions/ast";

import { Ast, generate, parse, rewrite } from "telarel";
import { Ast as AstFromEntry } from "telarel/ast";
import { describe, expect, it } from "vitest";

import { renameRootIdentifier } from "#/functions/ast";

type Program = AstOptions["program"];

type ReadonlyProgram = AstResult["ast"]["program"];

const makeProgram = (): Program =>
    ({
        type: "Program",
        sourceType: "module",
        body: [
            {
                type: "VariableDeclaration",
                kind: "const",
                declarations: [
                    {
                        type: "VariableDeclarator",
                        id: { type: "Identifier", name: "a" },
                        init: { type: "Literal", value: 1 },
                    },
                ],
            },
        ],
    }) as unknown as Program;

const rootName = (program: ReadonlyProgram): string => {
    const fixture: ProgramFixture = program as unknown as ProgramFixture;

    return fixture.body[0]?.declarations[0]?.id.name ?? "";
};

describe("Ast", (): void => {
    it("exposes the program, source, and map", (): void => {
        const program: Program = makeProgram();
        const source: string = "const a = 1;";
        const map: SourceMap = {
            version: 3,
            sources: ["index.ts"],
            sourcesContent: [source],
            names: [],
            mappings: "AAAA",
        };
        const ast: Ast = new Ast({ program, source, map });

        expect(ast.program).toBe(program);
        expect(ast.source).toBe(source);
        expect(ast.map).toBe(map);
    });

    it("returns null when no map is carried", (): void => {
        const ast: Ast = new Ast({
            program: makeProgram(),
            source: "const a = 1;",
        });

        expect(ast.map).toBeNull();
    });

    it("exposes a deeply read-only program", (): void => {
        const ast: Ast = new Ast({
            program: makeProgram(),
            source: "const a = 1;",
        });

        expect(ast.program.body).toHaveLength(1);

        // @ts-expect-error The program view is deeply read-only.
        ast.program.body = [];
    });

    it("mutates the program synchronously", (): void => {
        const ast: Ast = new Ast({
            program: makeProgram(),
            source: "const a = 1;",
        });

        const result: void = ast.mutate((program: Program): void => {
            renameRootIdentifier({
                program: program as unknown as ProgramFixture,
                name: "b",
            });
        });

        expect(result).toBeUndefined();
        expect(rootName(ast.program)).toBe("b");
    });

    it("returns the promise from an async mutation", async (): Promise<void> => {
        const ast: Ast = new Ast({
            program: makeProgram(),
            source: "const a = 1;",
        });

        const result: Promise<void> = ast.mutate(
            async (program: Program): Promise<void> => {
                await Promise.resolve();

                renameRootIdentifier({
                    program: program as unknown as ProgramFixture,
                    name: "c",
                });
            },
        );

        expect(result).toBeInstanceOf(Promise);

        await result;

        expect(rootName(ast.program)).toBe("c");
    });

    it("is returned by parse", async (): Promise<void> => {
        const result: AstResult = await parse("const a = 1;", {
            file: "index.ts",
        });

        expect(result.ast).toBeInstanceOf(Ast);
        expect(result.ast.source).toBe("const a = 1;");
        expect(result.ast.map).toBeNull();
    });

    it("has no language when constructed manually", (): void => {
        const ast: Ast = new Ast({
            program: makeProgram(),
            source: "const a = 1;",
        });

        expect(ast.language).toBeUndefined();
        expect(Object.keys(ast)).toEqual([]);
    });

    it("exposes sourceType as a read-through of the program", (): void => {
        const ast: Ast = new Ast({
            program: makeProgram(),
            source: "const a = 1;",
        });

        expect(ast.sourceType).toBe("module");
        expect(Object.keys(ast)).toEqual([]);

        ast.mutate((program: Program): void => {
            program.sourceType = "script";
        });

        expect(ast.sourceType).toBe("script");
        expect(ast.program.sourceType).toBe("script");
    });

    it("populates sourceType from parse", async (): Promise<void> => {
        const script: AstResult = await parse("const a = 1;", {
            file: "index.js",
        });
        const module: AstResult = await parse("export const a = 1;", {
            file: "index.js",
        });

        expect(script.ast.sourceType).toBe("script");
        expect(module.ast.sourceType).toBe("module");
    });

    it("exports Program and OxcProgram from telarel/ast", (): void => {
        const oxcProgram: OxcProgram = makeProgram();
        const program: AstProgram = oxcProgram;
        const ast: Ast = new Ast({
            program,
            source: "const a = 1;",
        });

        expect(oxcProgram.type).toBe("Program");
        expect(program.comments).toBeUndefined();
        expect(ast.program.type).toBe("Program");
    });

    it("exports the runtime Ast class from telarel/ast", (): void => {
        const program: Program = makeProgram();

        const ast: AstFromEntry = new AstFromEntry({
            program,
            source: "const a = 1;",
        });

        expect(ast).toBeInstanceOf(AstFromEntry);
        expect(AstFromEntry).toBe(Ast);
        expect(ast.program).toBe(program);
        expect(ast.source).toBe("const a = 1;");
        expect(ast.language).toBeUndefined();
        expect(ast.sourceType).toBe("module");
        expect(ast.map).toBeNull();
        expect(ast).toHaveProperty("mutate", expect.any(Function));

        ast.mutate((mutable: Program): void => {
            renameRootIdentifier({
                program: mutable as unknown as ProgramFixture,
                name: "b",
            });
        });

        expect(rootName(ast.program)).toBe("b");
    });

    it("passes the telarel/ast class to hooks and carries its mutation", async (): Promise<void> => {
        const parsed: AstResult = await parse("const a = 1;", {
            file: "index.ts",
        });

        let received: unknown;

        const rewritten: AstResult = await rewrite(parsed.ast, {
            file: "index.ts",
            plugins: [
                {
                    name: "rename",
                    transform: (
                        _ctx: PluginContext,
                        args: TransformArgs,
                    ): TransformResult => {
                        received = args.ast;

                        args.ast.mutate((program: Program): void => {
                            renameRootIdentifier({
                                program: program as unknown as ProgramFixture,
                                name: "b",
                            });
                        });

                        return { ast: args.ast };
                    },
                },
            ],
        });

        expect(received).toBeInstanceOf(AstFromEntry);

        const generated: CodeResult = await generate(rewritten.ast);

        expect(generated.code).toBe("const b = 1;");
    });

    it("rejects plain-object AST inputs", async (): Promise<void> => {
        await expect(
            rewrite(
                { program: makeProgram(), source: "const a = 1;" } as never,
                {
                    file: "index.ts",
                },
            ),
        ).rejects.toThrow("Expected an Ast instance");
    });
});
