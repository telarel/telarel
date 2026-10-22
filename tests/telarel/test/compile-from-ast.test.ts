import type {
    AstOptions,
    AstResult,
    CodeResult,
    CompileFromAstOptions,
    PluginContext,
    TransformArgs,
    TransformResult,
} from "telarel";

import type { ProgramFixture } from "#/functions/ast";

import { compileFromAst, parse } from "telarel";
import { describe, expect, it } from "vitest";

import { renameRootIdentifier } from "#/functions/ast";
import { buildMap } from "#/functions/sourcemap";

const parsed = async (): Promise<AstResult> =>
    await parse("const a = 1;", { file: "index.ts" });

describe("compileFromAst", (): void => {
    it("compiles an AST through the full chain", async (): Promise<void> => {
        const result: CodeResult = await compileFromAst((await parsed()).ast, {
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
            ],
        });

        expect(result.code).toBe("const renamed = 1;");
        expect(result.map?.version).toBe(3);
    });

    it("runs options, start, and end exactly once and skips parse", async (): Promise<void> => {
        const counts: {
            options: number;
            start: number;
            parse: number;
            end: number;
        } = { options: 0, start: 0, parse: 0, end: 0 };

        await compileFromAst((await parsed()).ast, {
            file: "index.ts",
            plugins: [
                {
                    name: "spy",
                    options: (): void => {
                        counts.options += 1;
                    },
                    start: (): void => {
                        counts.start += 1;
                    },
                    parse: (): null => {
                        counts.parse += 1;

                        return null;
                    },
                    end: (): void => {
                        counts.end += 1;
                    },
                },
            ],
        });

        expect(counts).toEqual({ options: 1, start: 1, parse: 0, end: 1 });
    });

    it("runs prepare, transform, finalize, and generate in order", async (): Promise<void> => {
        const seen: Array<string> = [];

        await compileFromAst((await parsed()).ast, {
            file: "index.ts",
            plugins: [
                {
                    name: "chain",
                    prepare: (): void => {
                        seen.push("prepare");
                    },
                    transform: (): void => {
                        seen.push("transform");
                    },
                    finalize: (): void => {
                        seen.push("finalize");
                    },
                    generate: (): null => {
                        seen.push("generate");

                        return null;
                    },
                },
            ],
        });

        expect(seen).toEqual(["prepare", "transform", "finalize", "generate"]);
    });

    it("appends the external map URL derived from output.file", async (): Promise<void> => {
        const result: CodeResult = await compileFromAst((await parsed()).ast, {
            file: "src/index.ts",
            output: { file: "dist/index.js" },
            sourceMap: { output: "external" },
        });

        expect(result.code).toBe(
            "const a = 1;\n//# sourceMappingURL=dist/index.js.map\n",
        );
        expect(result.map?.sources).toEqual(["src/index.ts"]);
    });

    it("accepts a bare program with its code", async (): Promise<void> => {
        const ast: AstResult = await parsed();

        const result: CodeResult = await compileFromAst(
            ast.ast.program as unknown as AstOptions["program"],
            ast.ast.source,
            { file: "index.ts" },
        );

        expect(result.code).toBe("const a = 1;");
        expect(result.map?.sources).toEqual(["index.ts"]);
    });

    it("composes a requested input map into the generated map", async (): Promise<void> => {
        const options: CompileFromAstOptions = {
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
        };

        const result: CodeResult = await compileFromAst(
            (await parsed()).ast,
            options,
        );

        expect(result.map?.file).toBe("index.ts");
        expect(result.map?.sources).toEqual(["index.ts"]);
        expect(result.map?.mappings).toBe("AASA,MAAM,IAAI");
    });
});
