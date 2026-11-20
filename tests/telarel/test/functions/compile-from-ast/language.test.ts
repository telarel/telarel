import type {
    AstResult,
    CodeResult,
    GenerateArgs,
    PluginContext,
    SourceType,
} from "telarel";

import { compileFromAst, parse } from "telarel";
import { describe, expect, it } from "vitest";

describe("compileFromAst language", (): void => {
    it("uses the AST language and lets an explicit language override it", async (): Promise<void> => {
        const tsx: AstResult = await parse("const f = <T,>(x: T) => x;", {
            language: "tsx",
        });

        const fromAst: CodeResult = await compileFromAst(tsx.ast);

        expect(fromAst.code).toBe("const f = <T,>(x: T) => x;");

        const explicit: CodeResult = await compileFromAst(tsx.ast, {
            language: "ts",
        });

        expect(explicit.code).toBe("const f = <T>(x: T) => x;");
    });

    it("keeps a commonjs AST when the file is mjs", async (): Promise<void> => {
        const commonjs: AstResult = await parse("module.exports = 1;", {
            file: "index.cjs",
        });
        const seen: Array<SourceType> = [];

        const result: CodeResult = await compileFromAst(commonjs.ast, {
            file: "index.mjs",
            plugins: [
                {
                    name: "observe-source-type",
                    generate: (
                        _ctx: PluginContext,
                        args: GenerateArgs,
                    ): null => {
                        seen.push(args.ast.program.sourceType);

                        return null;
                    },
                },
            ],
        });

        expect(result.code).toBe("module.exports = 1;");
        expect(seen).toEqual(["commonjs"]);
    });

    it("lets an explicit sourceType override the AST", async (): Promise<void> => {
        const moduleAst: AstResult = await parse("const a = 1;", {
            file: "index.mjs",
            sourceType: "module",
        });
        const seen: Array<string> = [];

        const result: CodeResult = await compileFromAst(moduleAst.ast, {
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

        expect(result.code).toBe("const a = 1;");
        expect(seen).toEqual(["script"]);
    });

    it("uses the AST module kind when sourceType is unambiguous", async (): Promise<void> => {
        const commonjs: AstResult = await parse("module.exports = 1;", {
            file: "index.cjs",
        });
        const seen: Array<SourceType> = [];

        await compileFromAst(commonjs.ast, {
            file: "index.mjs",
            sourceType: "unambiguous",
            plugins: [
                {
                    name: "observe-source-type",
                    generate: (
                        _ctx: PluginContext,
                        args: GenerateArgs,
                    ): null => {
                        seen.push(args.ast.program.sourceType);

                        return null;
                    },
                },
            ],
        });

        expect(seen).toEqual(["commonjs"]);
    });
});
