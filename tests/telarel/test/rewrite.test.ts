import type {
    AstOptions,
    AstResult,
    FinalizeArgs,
    Plugin,
    PluginContext,
    PrepareArgs,
    TransformArgs,
    TransformResult,
} from "telarel";

import type { ProgramFixture } from "#/functions/ast";

import { Ast, generate, parse, rewrite } from "telarel";
import { describe, expect, it } from "vitest";

import { renameRootIdentifier } from "#/functions/ast";
import { buildMap } from "#/functions/sourcemap";

const upstreamMap = (): ReturnType<typeof buildMap> =>
    buildMap({
        file: "upstream.ts",
        mappings: "AASA",
        names: [],
        sources: ["upstream.ts"],
        sourcesContent: ["const a = 1;"],
    });

const rootName = (ast: AstResult["ast"]): string => {
    const program: ProgramFixture = ast.program as unknown as ProgramFixture;

    return program.body[0]?.declarations[0]?.id.name ?? "";
};

const rename = (
    name: string,
): ((ctx: unknown, args: TransformArgs) => TransformResult) => {
    return (_ctx: unknown, args: TransformArgs): TransformResult => {
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

describe("rewrite", (): void => {
    it("rewrites an AST without touching the program", async (): Promise<void> => {
        const parsed: AstResult = await parse("const a = 1;", {
            file: "index.ts",
        });

        const result: AstResult = await rewrite(parsed.ast, {
            file: "index.ts",
        });

        expect(result.ast.source).toBe("const a = 1;");
        expect(result.ast.program.type).toBe("Program");
        expect(result.ast.map).toBeNull();
    });

    it("accepts a bare program with its code", async (): Promise<void> => {
        const parsed: AstResult = await parse("const a = 1;", {
            file: "index.ts",
        });

        const result: AstResult = await rewrite(
            parsed.ast.program as unknown as AstOptions["program"],
            parsed.ast.source,
            { file: "index.ts" },
        );

        expect(result.ast.program.type).toBe("Program");
        expect(result.ast.source).toBe("const a = 1;");
    });

    it("observes the AST in prepare and finalize with ignored returns", async (): Promise<void> => {
        const parsed: AstResult = await parse("const a = 1;", {
            file: "index.ts",
        });
        const prepareKeys: Array<Array<string>> = [];
        const finalizeKeys: Array<Array<string>> = [];

        const prepareObserver: unknown = {
            name: "prepare-observer",
            prepare: (_ctx: PluginContext, args: PrepareArgs): unknown => {
                prepareKeys.push(Object.keys(args));

                return { code: "const b = 2;" };
            },
        };

        const result: AstResult = await rewrite(parsed.ast, {
            file: "index.ts",
            plugins: [
                prepareObserver as Plugin,
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
        expect(rootName(result.ast)).toBe("a");
    });

    it("chains transforms return value to return value across plugins", async (): Promise<void> => {
        const parsed: AstResult = await parse("const a = 1;", {
            file: "index.ts",
        });

        const result: AstResult = await rewrite(parsed.ast, {
            file: "index.ts",
            plugins: [
                { name: "rename-a", transform: rename("first") },
                { name: "rename-b", transform: rename("second") },
            ],
        });

        expect(rootName(result.ast)).toBe("second");

        const generated = await generate(result.ast, { file: "index.ts" });

        expect(generated.code).toBe("const second = 1;");
    });

    it("does not fire the parse or generate hooks", async (): Promise<void> => {
        const parsed: AstResult = await parse("const a = 1;", {
            file: "index.ts",
        });
        const seen: Array<string> = [];

        await rewrite(parsed.ast, {
            file: "index.ts",
            plugins: [
                {
                    name: "stage-spy",
                    parse: (): null => {
                        seen.push("parse");

                        return null;
                    },
                    generate: (): null => {
                        seen.push("generate");

                        return null;
                    },
                },
            ],
        });

        expect(seen).toEqual([]);
    });

    it("attaches a requested input map when the AST carries none", async (): Promise<void> => {
        const parsed: AstResult = await parse("const a = 1;", {
            file: "index.ts",
        });

        const result: AstResult = await rewrite(parsed.ast, {
            file: "index.ts",
            sourceMap: { input: upstreamMap() },
        });

        expect(result.ast.map?.sources).toEqual(["upstream.ts"]);
        expect(result.ast.map?.mappings).toBe("AASA");
    });

    it("keeps the AST's own map over a requested input map", async (): Promise<void> => {
        const parsed: AstResult = await parse("const a = 1;", {
            file: "index.ts",
        });
        const ownMap = buildMap({
            file: "own.ts",
            mappings: "AAAA",
            names: [],
            sources: ["own.ts"],
        });
        const ast: Ast = new Ast({
            program: parsed.ast.program as unknown as AstOptions["program"],
            source: parsed.ast.source,
            map: ownMap,
        });

        const result: AstResult = await rewrite(ast, {
            file: "index.ts",
            sourceMap: { input: upstreamMap() },
        });

        expect(result.ast.map?.sources).toEqual(["own.ts"]);
        expect(result.ast.map?.mappings).toBe("AAAA");
    });

    it("rejects on a transform hook error", async (): Promise<void> => {
        const parsed: AstResult = await parse("const a = 1;", {
            file: "index.ts",
        });

        await expect(
            rewrite(parsed.ast, {
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
});
