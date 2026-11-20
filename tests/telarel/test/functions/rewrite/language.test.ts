import type {
    AstOptions,
    AstResult,
    CodeResult,
    FinalizeArgs,
    Language,
    PluginContext,
    TransformArgs,
    TransformResult,
    Ast,
} from "telarel";

import { generate, parse, rewrite } from "telarel";
import { describe, expect, it } from "vitest";

describe("rewrite language", (): void => {
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
});
