import type { AstOptions, AstResult, SourceMap } from "telarel";

import type { ProgramFixture } from "#/functions/ast";

import { Ast, parse, rewrite } from "telarel";
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
