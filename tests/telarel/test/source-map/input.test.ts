import type {
    AstOptions,
    AstResult,
    CodeResult,
    DeepReadonly,
    ParseArgs,
    PluginContext,
    Program,
    SourceMap,
    TransformArgs,
    TransformResult,
} from "telarel";

import type { ProgramFixture } from "#/functions/ast";

import * as Fsp from "node:fs/promises";
import * as Path from "node:path";

import { Ast, compile, parse } from "telarel";
import { afterEach, describe, expect, it } from "vitest";

import { renameRootIdentifier } from "#/functions/ast";
import {
    appendInlineMap,
    buildMap,
    countSourceMappingComments,
    firstMappingSegment,
    makeTempDir,
    removeTempDir,
    SOURCE_MAPPING_COMMENT,
} from "#/functions/sourcemap";

const tempDirs: Array<string> = [];

// NAPI-RS preopens the drive root as a guest path such as `D:\`, but
// wasi-libc recognizes only `/` separators. An input map on the filesystem
// cannot resolve a Windows absolute path under WASI. Therefore, the
// external-map tests skip on Windows WASI.
const isWasiWindows: boolean =
    process.env.NAPI_RS_FORCE_WASI === "error" && process.platform === "win32";

const tempDir = async (): Promise<string> => {
    const directory: string = await makeTempDir("telarel-sourcemap-");

    tempDirs.push(directory);

    return directory;
};

afterEach(async (): Promise<void> => {
    await Promise.all(tempDirs.splice(0).map(removeTempDir));
});

const upstreamMap = (): ReturnType<typeof buildMap> =>
    buildMap({
        file: "upstream.ts",
        mappings: "AASA",
        names: [],
        sources: ["upstream.ts"],
        sourcesContent: ["const a = 1;"],
    });

const ownMap = (): ReturnType<typeof buildMap> =>
    buildMap({
        file: "own.ts",
        mappings: "AAAA",
        names: [],
        sources: ["own.ts"],
        sourcesContent: ["const a = 1;"],
    });

describe("source map input", (): void => {
    it("passes the Ast source and input map to transform", async (): Promise<void> => {
        const code: string = "const a = 1;";
        const inputMap: SourceMap = buildMap({
            file: "upstream.ts",
            mappings: "AASA",
            names: [],
            sources: ["upstream.ts"],
            sourcesContent: [code],
        });
        let observedCode: string = "";
        const observedMaps: Array<DeepReadonly<SourceMap> | null> = [];

        await compile(code, {
            cwd: "/repo",
            file: "index.ts",
            sourceMap: { input: inputMap },
            plugins: [
                {
                    name: "ast-observer",
                    transform: (
                        _ctx: PluginContext,
                        args: TransformArgs,
                    ): TransformResult => {
                        observedCode = args.ast.source;
                        observedMaps.push(args.ast.map);

                        return { ast: args.ast };
                    },
                },
            ],
        });

        expect(observedCode).toBe(code);
        expect(observedMaps[0]?.sources).toEqual(["upstream.ts"]);
        expect(observedMaps[0]?.mappings).toBe("AASA");
    });

    it("keeps the map stable after a transform mutation", async (): Promise<void> => {
        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "renamer",
                    transform: (
                        _ctx: PluginContext,
                        args: TransformArgs,
                    ): TransformResult => {
                        args.ast.mutate((program: Program): void => {
                            renameRootIdentifier({
                                program: program as unknown as ProgramFixture,
                                name: "renamed",
                            });
                        });

                        return { ast: args.ast };
                    },
                },
            ],
        });

        expect(result.code).toBe("const renamed = 1;");
        expect(result.map?.version).toBe(3);
        expect(result.map?.file).toBe("index.ts");
        expect(result.map?.sources).toEqual(["index.ts"]);
        expect(result.map?.mappings.length).toBeGreaterThan(0);
    });

    it("composes a multi-source input map without delta offsets", async (): Promise<void> => {
        const result: CodeResult = await compile("const a = 1;\nconst b = 2;", {
            cwd: "/repo",
            file: "index.ts",
            sourceMap: {
                input: buildMap({
                    file: "bundle.ts",
                    mappings: "AASA,UCWK;AAoBH,UDUK",
                    names: [],
                    sourceRoot: "src",
                    ignoreList: [1],
                    sources: ["a.ts", "b.ts"],
                    sourcesContent: ["aaa", "bbb"],
                }),
            },
        });

        // Input segments are non-uniform across two sources. A codegen token
        // between segments takes the segment's original position and source,
        // instead of inheriting the codegen map's source and a delta offset.
        expect(result.map?.file).toBe("index.ts");
        expect(result.map?.sourceRoot).toBe("src");
        expect(result.map?.sources).toEqual(["a.ts", "b.ts"]);
        expect(result.map?.sourcesContent).toEqual(["aaa", "bbb"]);
        expect(result.map?.ignoreList).toEqual([1]);
        expect(result.map?.mappings).toBe("AASA,MAAA,ICWK;AAoBH,MAAA,IDUK");
    });

    it("replaces a winning parse hook AST's own input map with the requested inline map", async (): Promise<void> => {
        const code: string = appendInlineMap("const a = 1;", upstreamMap());
        const astMap: SourceMap = ownMap();

        const parsed: AstResult = await parse(code, {
            cwd: "/repo",
            file: "index.ts",
            inputSourceMap: astMap,
        });

        expect(parsed.ast.map?.sources).toEqual(["own.ts"]);
        expect(parsed.ast.source).toContain(SOURCE_MAPPING_COMMENT);

        const result: CodeResult = await compile(code, {
            cwd: "/repo",
            file: "index.ts",
            sourceMap: { input: "inline" },
            plugins: [
                {
                    name: "own-map",
                    parse: (
                        _ctx: PluginContext,
                        _args: ParseArgs,
                    ): AstResult => ({ ast: parsed.ast }),
                },
            ],
        });

        expect(result.code).toBe("const a = 1;");
        expect(countSourceMappingComments(result.code)).toBe(0);
        expect(result.map?.sources).toEqual(["upstream.ts"]);
        expect(firstMappingSegment(result.map?.mappings ?? "")).toEqual({
            dstColumn: 0,
            sourceIndex: 0,
            sourceLine: 9,
            sourceColumn: 0,
        });
    });

    it("keeps a transform-returned map over the boundary request", async (): Promise<void> => {
        const transformMap: SourceMap = buildMap({
            file: "transform.ts",
            mappings: "AAKA",
            names: [],
            sources: ["transform.ts"],
            sourcesContent: ["const a = 1;"],
        });
        let observedSources: ReadonlyArray<string> | undefined;

        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            sourceMap: { input: upstreamMap() },
            plugins: [
                {
                    name: "transform-map",
                    transform: (
                        _ctx: PluginContext,
                        args: TransformArgs,
                    ): TransformResult => {
                        observedSources = args.ast.map?.sources;

                        return {
                            ast: new Ast({
                                program: args.ast
                                    .program as unknown as AstOptions["program"],
                                source: args.ast.source,
                                map: transformMap,
                            }),
                        };
                    },
                },
            ],
        });

        expect(observedSources).toEqual(["upstream.ts"]);
        expect(result.code).toBe("const a = 1;");
        expect(result.map?.sources).toEqual(["transform.ts"]);
        expect(firstMappingSegment(result.map?.mappings ?? "")).toEqual({
            dstColumn: 0,
            sourceIndex: 0,
            sourceLine: 5,
            sourceColumn: 0,
        });
    });

    it.skipIf(isWasiWindows)(
        "keeps a non-data sourceMappingURL comment when auto falls back to an external map",
        async (): Promise<void> => {
            const directory: string = await tempDir();
            const external: SourceMap = buildMap({
                file: "external.ts",
                mappings: "AASA",
                names: [],
                sources: ["external.ts"],
                sourcesContent: ["const a = 1;"],
            });

            await Fsp.writeFile(
                Path.join(directory, "index.ts.map"),
                JSON.stringify(external),
                "utf-8",
            );

            const lookalike: string = `${SOURCE_MAPPING_COMMENT}docs.map`;
            const code: string = `const a = 1;\n${lookalike}\n`;

            const result: CodeResult = await compile(code, {
                cwd: directory,
                file: "index.ts",
                sourceMap: { input: "auto" },
            });

            expect(result.code).toContain(lookalike);
            expect(countSourceMappingComments(result.code)).toBe(1);
            expect(result.map?.sources).toEqual(["external.ts"]);
        },
    );
});
