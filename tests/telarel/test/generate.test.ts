import type {
    Ast,
    AstOptions,
    AstResult,
    CodeResult,
    GenerateArgs,
    Generator,
    Language,
    PluginContext,
    SourceMap,
} from "telarel";

import { CompileError, createGenerator, generate, parse } from "telarel";
import { describe, expect, it } from "vitest";

import { buildMap, SOURCE_MAPPING_COMMENT } from "#/functions/sourcemap";

const parsed = async (): Promise<AstResult> =>
    await parse("const a = 1;", { file: "index.ts" });

const decodeInlineMap = (code: string): SourceMap => {
    const payload: string = code.split("base64,")[1]?.trim() ?? "";
    const json: string = Buffer.from(payload, "base64").toString("utf-8");

    return JSON.parse(json) as SourceMap;
};

describe("generate", (): void => {
    it("generates from an AST with the default external map", async (): Promise<void> => {
        const result: CodeResult = await generate((await parsed()).ast, {
            file: "index.ts",
        });

        expect(result.code).toBe("const a = 1;");
        expect(result.map?.version).toBe(3);
        expect(result.map?.sources).toEqual(["index.ts"]);
        expect(result.map?.mappings).toBe("AAAA,MAAM,IAAI");
        expect(result.code).not.toContain(SOURCE_MAPPING_COMMENT);
    });

    it("appends a configured external map URL and keeps the map", async (): Promise<void> => {
        const result: CodeResult = await generate((await parsed()).ast, {
            file: "index.ts",
            sourceMap: {
                output: {
                    mode: "external",
                    url: "dist/index.js.map",
                },
            },
        });

        expect(result.code).toBe(
            "const a = 1;\n//# sourceMappingURL=dist/index.js.map\n",
        );
        expect(result.map?.sources).toEqual(["index.ts"]);
    });

    it("uses output.file for the default external map URL", async (): Promise<void> => {
        const result: CodeResult = await generate((await parsed()).ast, {
            file: "index.ts",
            output: {
                file: "dist/index.js",
            },
            sourceMap: { output: "external" },
        });

        expect(result.code).toBe(
            "const a = 1;\n//# sourceMappingURL=dist/index.js.map\n",
        );
        expect(result.map?.sources).toEqual(["index.ts"]);
    });

    it("falls back to file for the default external map URL", async (): Promise<void> => {
        const result: CodeResult = await generate((await parsed()).ast, {
            file: "index.ts",
            sourceMap: { output: "external" },
        });

        expect(result.code).toBe(
            "const a = 1;\n//# sourceMappingURL=index.ts.map\n",
        );
        expect(result.map?.sources).toEqual(["index.ts"]);
    });

    it("omits the comment when sourceMap.output is not requested", async (): Promise<void> => {
        const result: CodeResult = await generate((await parsed()).ast, {
            file: "index.ts",
            output: { file: "dist/index.js" },
        });

        expect(result.code).toBe("const a = 1;");
        expect(result.map?.sources).toEqual(["index.ts"]);
    });

    it("generates from a bare program with its code", async (): Promise<void> => {
        const ast: AstResult = await parsed();

        const result: CodeResult = await generate(
            ast.ast.program as unknown as AstOptions["program"],
            ast.ast.source,
            {
                file: "index.ts",
            },
        );

        expect(result.code).toBe("const a = 1;");
        expect(result.map?.sources).toEqual(["index.ts"]);
    });

    it("appends an inline map and drops the map in inline mode", async (): Promise<void> => {
        const result: CodeResult = await generate((await parsed()).ast, {
            file: "index.ts",
            sourceMap: { output: "inline" },
        });

        expect(result.map).toBeNull();
        expect(result.code).toContain(SOURCE_MAPPING_COMMENT);
        expect(decodeInlineMap(result.code).sources).toEqual(["index.ts"]);
    });

    it("ignores the external map URL in inline mode", async (): Promise<void> => {
        const result: CodeResult = await generate((await parsed()).ast, {
            file: "index.ts",
            sourceMap: {
                output: {
                    mode: "inline",
                    url: "dist/index.js.map",
                },
            },
        });

        expect(result.map).toBeNull();
        expect(result.code).toContain(SOURCE_MAPPING_COMMENT);
        expect(result.code).toContain("data:application/json;base64,");
        expect(result.code).not.toContain("sourceMappingURL=dist/index.js.map");
    });

    it("accepts the output options object form", async (): Promise<void> => {
        const result: CodeResult = await generate((await parsed()).ast, {
            file: "index.ts",
            sourceMap: { output: { mode: "inline" } },
        });

        expect(result.map).toBeNull();
        expect(result.code).toContain(SOURCE_MAPPING_COMMENT);
    });

    it("appends the comment and keeps the map in both mode", async (): Promise<void> => {
        const result: CodeResult = await generate((await parsed()).ast, {
            file: "index.ts",
            sourceMap: { output: "both" },
        });

        expect(result.map?.sources).toEqual(["index.ts"]);
        expect(result.code).toContain(SOURCE_MAPPING_COMMENT);
    });

    it("uses the AST language when no generation language is given", async (): Promise<void> => {
        const ast: AstResult["ast"] = (
            await parse("const f = <T,>(x: T) => x;", { language: "tsx" })
        ).ast;

        const result: CodeResult = await generate(ast);

        expect(ast.language).toBe("tsx");
        expect(result.code).toBe("const f = <T,>(x: T) => x;");
    });

    it("lets an explicit language override the AST language", async (): Promise<void> => {
        const ast: AstResult["ast"] = (
            await parse("const f = <T,>(x: T) => x;", { language: "tsx" })
        ).ast;

        const result: CodeResult = await generate(ast, { language: "ts" });

        expect(result.code).toBe("const f = <T>(x: T) => x;");
    });

    it("createGenerator prefers the AST language over the file", async (): Promise<void> => {
        const ast: Ast = (
            await parse("const f = <T,>(x: T) => x;", {
                language: "tsx",
            })
        ).ast;
        const generator: Generator = createGenerator(ast, {
            cwd: "/repo",
            file: "index.ts",
            sourceMap: {},
        });
        const result: CodeResult = await generator.generate();

        expect(result.code).toBe("const f = <T,>(x: T) => x;");
    });

    it("exposes the AST language to the generate hook", async (): Promise<void> => {
        const ast: AstResult["ast"] = (
            await parse("const a: number = 1;", { file: "index.ts" })
        ).ast;
        const seen: Array<Language | void> = [];

        await generate(ast, {
            plugins: [
                {
                    name: "observe-language",
                    generate: (
                        _ctx: PluginContext,
                        args: GenerateArgs,
                    ): null => {
                        seen.push(args.ast.language);

                        return null;
                    },
                },
            ],
        });

        expect(seen).toEqual(["ts"]);
    });

    it("exposes the AST language to plugin contexts", async (): Promise<void> => {
        const ast: Ast = (
            await parse("const f = <T,>(x: T) => x;", {
                language: "tsx",
            })
        ).ast;

        await generate(ast, {
            file: "index.ts",
            plugins: [
                {
                    name: "observe-language",
                    generate: (ctx: PluginContext): null => {
                        expect(ctx.module.language).toBe("tsx");
                        return null;
                    },
                },
            ],
        });
    });

    it("exposes the ast and options to the generate hook", async (): Promise<void> => {
        const seen: Array<{
            keys: Array<string>;
            astKeys: Array<string>;
            file: string;
        }> = [];

        await generate((await parsed()).ast, {
            file: "index.ts",
            plugins: [
                {
                    name: "observe-generate",
                    generate: (
                        _ctx: PluginContext,
                        args: GenerateArgs,
                    ): null => {
                        seen.push({
                            keys: Object.keys(args),
                            astKeys: Object.keys(args.ast),
                            file: args.options.file,
                        });

                        return null;
                    },
                },
            ],
        });

        expect(seen).toEqual([
            {
                keys: ["ast", "options"],
                astKeys: [],
                file: "index.ts",
            },
        ]);
    });

    it("lets the first generate hook win and ignores later hooks", async (): Promise<void> => {
        const seen: Array<string> = [];

        const result: CodeResult = await generate((await parsed()).ast, {
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

    it("defers a null generate hook to the builtin generator", async (): Promise<void> => {
        const seen: Array<string> = [];

        const result: CodeResult = await generate((await parsed()).ast, {
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

    it("carries a generate hook's returned map through", async (): Promise<void> => {
        const map: SourceMap = buildMap({
            file: "index.ts",
            mappings: "AAAA",
            names: [],
            sources: ["index.ts"],
            sourcesContent: ["const a = 1;"],
        });

        const result: CodeResult = await generate((await parsed()).ast, {
            file: "index.ts",
            plugins: [
                {
                    name: "gen-map",
                    generate: (): CodeResult => ({ code: "custom();", map }),
                },
            ],
        });

        expect(result.code).toBe("custom();");
        expect(result.map?.mappings).toBe("AAAA");
        expect(result.map?.sources).toEqual(["index.ts"]);
    });

    it("returns no map when a generate hook supplies none", async (): Promise<void> => {
        const result: CodeResult = await generate((await parsed()).ast, {
            file: "index.ts",
            plugins: [
                {
                    name: "gen-mapless",
                    generate: (): CodeResult => ({ code: "mapless();" }),
                },
            ],
        });

        expect(result.code).toBe("mapless();");
        expect(result.map).toBeNull();
    });

    it("surfaces a generate hook error unchanged", async (): Promise<void> => {
        await expect(
            generate((await parsed()).ast, {
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

    it("rejects an invalid language with a CompileError", async (): Promise<void> => {
        const ast: Ast = (await parse("const a = 1;")).ast;

        await expect(
            generate(ast, { language: "json" as never }),
        ).rejects.toSatisfy(
            (error: unknown): boolean =>
                error instanceof CompileError &&
                error.message ===
                    'invalid language "json": expected "js", "ts", "dts", "jsx" or "tsx"',
        );
    });

    it("rejects a malformed output mode", async (): Promise<void> => {
        await expect(
            generate((await parsed()).ast, {
                file: "index.ts",
                sourceMap: { output: "sideways" as never },
            }),
        ).rejects.toThrow("invalid sourceMap.output mode");
    });
});
