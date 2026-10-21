import type {
    AstOptions,
    AstResult,
    CodeResult,
    GenerateArgs,
    PluginContext,
    SourceMap,
} from "telarel";

import { generate, parse } from "telarel";
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
        // The external mode returns the map without appending a comment.
        expect(result.code).not.toContain(SOURCE_MAPPING_COMMENT);
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

    it("rejects a malformed output mode", async (): Promise<void> => {
        await expect(
            generate((await parsed()).ast, {
                file: "index.ts",
                sourceMap: { output: "sideways" as never },
            }),
        ).rejects.toThrow("invalid sourceMap.output mode");
    });
});
