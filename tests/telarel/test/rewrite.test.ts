import type {
    AstOptions,
    AstResult,
    CodeResult,
    CommonPluginContext,
    FinalizeArgs,
    Language,
    OptionsArgs,
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

    it("uses the AST language and lets an explicit language override it", async (): Promise<void> => {
        const parsed: AstResult = await parse("const a = 1;", {
            file: "index.ts",
        });

        expect(parsed.ast.language).toBe("ts");

        const carried: AstResult = await rewrite(parsed.ast);

        expect(carried.ast.language).toBe("ts");

        const overridden: AstResult = await rewrite(parsed.ast, {
            file: "index.js",
            language: "js",
        });

        expect(overridden.ast.language).toBe("js");
    });

    it("uses a transform-returned AST language for the result and generation", async (): Promise<void> => {
        const replacement: Ast = (
            await parse("const f = <T,>(x: T) => x;", { file: "index.tsx" })
        ).ast;
        const seen: Array<Language | void> = [];

        const result: AstResult = await rewrite(
            (await parse("const a = 1;", { file: "index.js" })).ast,
            {
                cwd: "/repo",
                file: "index.js",
                plugins: [
                    {
                        name: "replace-with-tsx",
                        transform: (
                            _ctx: PluginContext,
                            args: TransformArgs,
                        ): TransformResult => {
                            expect(args.ast.language).toBe("js");

                            return { ast: replacement };
                        },
                        finalize: (
                            _ctx: PluginContext,
                            args: FinalizeArgs,
                        ): void => {
                            seen.push(args.ast.language);
                        },
                    },
                ],
            },
        );

        expect(seen).toEqual(["tsx"]);
        expect(result.ast.language).toBe("tsx");

        const generated: CodeResult = await generate(result.ast);

        expect(generated.code).toBe("const f = <T,>(x: T) => x;");
    });

    it("exposes the AST language to hook contexts", async (): Promise<void> => {
        const ast: Ast = (
            await parse("const f = <T,>(x: T) => x;", {
                language: "tsx",
            })
        ).ast;

        await rewrite(ast, {
            file: "index.ts",
            plugins: [
                {
                    name: "observe-language",
                    prepare: (ctx: PluginContext): void => {
                        expect(ctx.module.language).toBe("tsx");
                    },
                },
            ],
        });
    });

    it("honors a transform-mutated program source type", async (): Promise<void> => {
        const parsed: AstResult = await parse("const a = 1;", {
            file: "index.ts",
            sourceType: "module",
        });

        expect(parsed.ast.program.sourceType).toBe("module");

        const result: AstResult = await rewrite(parsed.ast, {
            file: "index.ts",
            plugins: [
                {
                    name: "mutate-source-type",
                    transform: (
                        _ctx: PluginContext,
                        args: TransformArgs,
                    ): TransformResult => {
                        args.ast.mutate(
                            (program: AstOptions["program"]): void => {
                                program.sourceType = "script";
                            },
                        );

                        return { ast: args.ast };
                    },
                },
            ],
        });

        expect(result.ast.program.sourceType).toBe("script");
    });

    it("keeps a module AST when the file is cjs", async (): Promise<void> => {
        const parsed: AstResult = await parse("const a = 1;", {
            file: "index.mjs",
            sourceType: "module",
        });
        const seen: Array<string> = [];

        const result: AstResult = await rewrite(parsed.ast, {
            file: "index.cjs",
            plugins: [
                {
                    name: "observe-source-type",
                    prepare: (ctx: PluginContext): void => {
                        seen.push(ctx.module.sourceType);
                    },
                },
            ],
        });

        expect(result.ast.program.sourceType).toBe("module");
        expect(seen).toEqual(["module"]);
    });

    it("keeps a commonjs AST when the file is mjs", async (): Promise<void> => {
        const parsed: AstResult = await parse("module.exports = 1;", {
            file: "index.cjs",
        });

        const result: AstResult = await rewrite(parsed.ast, {
            file: "index.mjs",
        });

        expect(result.ast.program.sourceType).toBe("commonjs");
    });

    it("lets an explicit sourceType override the AST", async (): Promise<void> => {
        const parsed: AstResult = await parse("const a = 1;", {
            file: "index.mjs",
            sourceType: "module",
        });
        const seen: Array<string> = [];

        const result: AstResult = await rewrite(parsed.ast, {
            file: "index.cjs",
            sourceType: "script",
            plugins: [
                {
                    name: "observe-source-type",
                    prepare: (ctx: PluginContext): void => {
                        seen.push(ctx.module.sourceType);
                    },
                },
            ],
        });

        expect(result.ast.program.sourceType).toBe("script");
        expect(seen).toEqual(["script"]);
    });

    it("uses the AST module kind when sourceType is unambiguous", async (): Promise<void> => {
        const parsed: AstResult = await parse("module.exports = 1;", {
            file: "index.cjs",
        });

        const result: AstResult = await rewrite(parsed.ast, {
            file: "index.mjs",
            sourceType: "unambiguous",
        });

        expect(result.ast.program.sourceType).toBe("commonjs");
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
            inputSourceMap: upstreamMap(),
        });

        expect(result.ast.map?.sources).toEqual(["upstream.ts"]);
        expect(result.ast.map?.mappings).toBe("AASA");
    });

    it("ignores an output map set by an options hook", async (): Promise<void> => {
        const parsed: AstResult = await parse("const a = 1;", {
            file: "index.ts",
        });

        const result: AstResult = await rewrite(parsed.ast, {
            file: "index.ts",
            inputSourceMap: upstreamMap(),
            plugins: [
                {
                    name: "output-for-every-stage",
                    options: (
                        _ctx: CommonPluginContext,
                        args: OptionsArgs,
                    ): OptionsArgs => ({
                        options: {
                            ...args.options,
                            sourceMap: {
                                ...args.options.sourceMap,
                                output: "inline",
                            },
                        },
                    }),
                },
            ],
        });

        expect(result.ast.map?.sources).toEqual(["upstream.ts"]);
    });

    it("ignores a top-level output set by an options hook", async (): Promise<void> => {
        const parsed: AstResult = await parse("const a = 1;", {
            file: "index.ts",
        });

        const result: AstResult = await rewrite(parsed.ast, {
            file: "index.ts",
            inputSourceMap: upstreamMap(),
            plugins: [
                {
                    name: "set-top-level-output",
                    options: (
                        _ctx: CommonPluginContext,
                        args: OptionsArgs,
                    ): OptionsArgs => ({
                        options: {
                            ...args.options,
                            output: { file: "dist/index.js" },
                        },
                    }),
                },
            ],
        });

        expect(result.ast.source).toBe("const a = 1;");
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
            inputSourceMap: upstreamMap(),
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
