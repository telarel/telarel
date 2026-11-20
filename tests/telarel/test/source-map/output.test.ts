// The output-path tests assert through `expectExternalMap`.
// oxlint-disable vitest/expect-expect

import type {
    CodeResult,
    CommonPluginContext,
    OptionsArgs,
    PluginContext,
    SourceMap,
    SourceMapOptions,
    StartArgs,
    Ast,
} from "telarel";

import * as Fsp from "node:fs/promises";
import * as Path from "node:path";

import {
    compile,
    compileFromAst,
    createGenerator,
    generate,
    parse,
} from "telarel";
import { afterEach, describe, expect, it } from "vitest";

import { resolvedGeneratorOptions } from "#/functions/options";
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

const astWithOwnMap = async (): Promise<Ast> =>
    (
        await parse("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            inputSourceMap: ownMap(),
        })
    ).ast;

const plainAst = async (): Promise<Ast> =>
    (
        await parse("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
        })
    ).ast;

type ExternalMapExpectation = {
    source: string;
    url: string;
};

const expectExternalMap = (
    result: CodeResult,
    expected: ExternalMapExpectation,
): void => {
    expect(result.code).toBe(
        `const a = 1;\n${SOURCE_MAPPING_COMMENT}${expected.url}\n`,
    );
    expect(result.map).not.toBeNull();
    expect(result.map?.version).toBe(3);
    expect(result.map?.sources).toEqual([expected.source]);
    expect(result.map?.mappings).toBe("AAAA,MAAM,IAAI");
};

describe("source map output", (): void => {
    it("passes source-map options through the options bag", async (): Promise<void> => {
        const seen: Array<SourceMapOptions | undefined> = [];

        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            output: {
                file: "dist/index.js",
            },
            sourceMap: {
                input: { mode: "inline", path: "ignored.map" },
                output: {
                    mode: "external",
                    url: "dist/index.js.map",
                },
            },
            plugins: [
                {
                    name: "observe-options",
                    options: (
                        _ctx: CommonPluginContext,
                        args: OptionsArgs,
                    ): void => {
                        seen.push(args.options.sourceMap);
                    },
                },
            ],
        });

        expect(seen).toEqual([
            {
                input: { mode: "inline", path: "ignored.map" },
                output: {
                    mode: "external",
                    url: "dist/index.js.map",
                },
            },
        ]);

        expect(result.code).toBe(
            "const a = 1;\n//# sourceMappingURL=dist/index.js.map\n",
        );
    });

    it("passes output.file through the options bag", async (): Promise<void> => {
        const seen: Array<{ file?: string } | undefined> = [];

        await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            output: { file: "dist/index.js" },
            plugins: [
                {
                    name: "observe-output",
                    options: (
                        _ctx: CommonPluginContext,
                        args: OptionsArgs,
                    ): void => {
                        seen.push(args.options.output);
                    },
                },
            ],
        });

        expect(seen).toEqual([{ file: "dist/index.js" }]);
    });

    it("passes the resolved output.file to start", async (): Promise<void> => {
        let seen: string | undefined;

        await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            output: { file: "dist/index.js" },
            plugins: [
                {
                    name: "observe-start-output",
                    start: (_ctx: PluginContext, args: StartArgs): void => {
                        seen = args.options.output.file;
                    },
                },
            ],
        });

        expect(seen).toBe("dist/index.js");
    });

    it("lets an options hook set output.file", async (): Promise<void> => {
        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            sourceMap: { output: "external" },
            plugins: [
                {
                    name: "set-output",
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

        expect(result.code).toBe(
            "const a = 1;\n//# sourceMappingURL=dist/index.js.map\n",
        );
    });

    it("treats an empty output.file as unset for the external map URL", async (): Promise<void> => {
        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            output: { file: "" },
            sourceMap: { output: "external" },
        });

        expect(result.code).toBe(
            "const a = 1;\n//# sourceMappingURL=index.ts.map\n",
        );
    });

    it("falls back to output.file when sourceMap.output.url is empty", async (): Promise<void> => {
        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            output: { file: "dist/index.js" },
            sourceMap: {
                output: {
                    mode: "external",
                    url: "",
                },
            },
        });

        expect(result.code).toBe(
            "const a = 1;\n//# sourceMappingURL=dist/index.js.map\n",
        );
    });
});

describe("createGenerator source maps", (): void => {
    it("records file as map.file", async (): Promise<void> => {
        const ast: Ast = await plainAst();

        const result: CodeResult = await createGenerator(
            ast,
            resolvedGeneratorOptions({ file: "src/index.ts" }),
        ).generate();

        expect(result.map?.file).toBe("src/index.ts");
        expect(result.map?.sources).toEqual(["src/index.ts"]);
    });

    it("overrides the AST's own map with a value input request", async (): Promise<void> => {
        const ast: Ast = await astWithOwnMap();

        expect(ast.map?.sources).toEqual(["own.ts"]);

        const result: CodeResult = await createGenerator(
            ast,
            resolvedGeneratorOptions({
                sourceMap: { input: upstreamMap() },
            }),
        ).generate();

        expect(result.map?.sources).toEqual(["upstream.ts"]);
        expect(firstMappingSegment(result.map?.mappings ?? "")).toEqual({
            dstColumn: 0,
            sourceIndex: 0,
            sourceLine: 9,
            sourceColumn: 0,
        });
    });

    it("resolves an inline input map and strips the consumed comment", async (): Promise<void> => {
        const code: string = appendInlineMap("const a = 1;", upstreamMap());
        const ast: Ast = (
            await parse(code, {
                cwd: "/repo",
                file: "index.ts",
                inputSourceMap: ownMap(),
            })
        ).ast;

        expect(ast.source).toContain(SOURCE_MAPPING_COMMENT);

        const result: CodeResult = await createGenerator(
            ast,
            resolvedGeneratorOptions({ sourceMap: { input: "inline" } }),
        ).generate();

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

    it.skipIf(isWasiWindows)(
        "reads an external input map through cwd",
        async (): Promise<void> => {
            const directory: string = await tempDir();
            const externalMap: SourceMap = buildMap({
                file: "external.ts",
                mappings: "AAMA",
                names: [],
                sources: ["external.ts"],
                sourcesContent: ["const a = 1;"],
            });

            await Fsp.writeFile(
                Path.join(directory, "index.ts.map"),
                JSON.stringify(externalMap),
                "utf-8",
            );

            const ast: Ast = (
                await parse("const a = 1;", {
                    cwd: "/repo",
                    file: "index.ts",
                })
            ).ast;
            const result: CodeResult = await createGenerator(
                ast,
                resolvedGeneratorOptions({
                    cwd: directory,
                    sourceMap: { input: "external" },
                }),
            ).generate();

            expect(result.map?.sources).toEqual(["external.ts"]);
            expect(firstMappingSegment(result.map?.mappings ?? "")).toEqual({
                dstColumn: 0,
                sourceIndex: 0,
                sourceLine: 6,
                sourceColumn: 0,
            });
        },
    );

    it.skipIf(isWasiWindows)(
        "prefers an inline map and falls back to an external map in auto mode",
        async (): Promise<void> => {
            const directory: string = await tempDir();
            const externalMap: SourceMap = buildMap({
                file: "external.ts",
                mappings: "AAMA",
                names: [],
                sources: ["external.ts"],
                sourcesContent: ["const a = 1;"],
            });

            await Fsp.writeFile(
                Path.join(directory, "index.ts.map"),
                JSON.stringify(externalMap),
                "utf-8",
            );

            const inlineCode: string = appendInlineMap(
                "const a = 1;",
                buildMap({
                    file: "inline.ts",
                    mappings: "AAIA",
                    names: [],
                    sources: ["inline.ts"],
                    sourcesContent: ["const a = 1;"],
                }),
            );
            const inlineAst: Ast = (
                await parse(inlineCode, {
                    cwd: "/repo",
                    file: "index.ts",
                })
            ).ast;
            const inlineResult: CodeResult = await createGenerator(
                inlineAst,
                resolvedGeneratorOptions({
                    cwd: directory,
                    sourceMap: { input: "auto" },
                }),
            ).generate();

            expect(inlineResult.map?.sources).toEqual(["inline.ts"]);
            expect(
                firstMappingSegment(inlineResult.map?.mappings ?? ""),
            ).toEqual({
                dstColumn: 0,
                sourceIndex: 0,
                sourceLine: 4,
                sourceColumn: 0,
            });

            const externalAst: Ast = (
                await parse("const a = 1;", {
                    cwd: "/repo",
                    file: "index.ts",
                })
            ).ast;
            const externalResult: CodeResult = await createGenerator(
                externalAst,
                resolvedGeneratorOptions({
                    cwd: directory,
                    sourceMap: { input: "auto" },
                }),
            ).generate();

            expect(externalResult.map?.sources).toEqual(["external.ts"]);
            expect(
                firstMappingSegment(externalResult.map?.mappings ?? ""),
            ).toEqual({
                dstColumn: 0,
                sourceIndex: 0,
                sourceLine: 6,
                sourceColumn: 0,
            });
        },
    );

    it("keeps the AST's own map when a best-effort input request finds no map", async (): Promise<void> => {
        const directory: string = await tempDir();
        const ast: Ast = await astWithOwnMap();

        for (const input of ["inline", "auto"] as const) {
            const result: CodeResult = await createGenerator(
                ast,
                resolvedGeneratorOptions({
                    cwd: directory,
                    sourceMap: { input },
                }),
            ).generate();

            expect(result.map?.sources).toEqual(["own.ts"]);
            expect(firstMappingSegment(result.map?.mappings ?? "")).toEqual({
                dstColumn: 0,
                sourceIndex: 0,
                sourceLine: 0,
                sourceColumn: 0,
            });
        }
    });

    it("returns no map and appends exactly one inline comment for output inline", async (): Promise<void> => {
        const ast: Ast = (
            await parse("const a = 1;", {
                cwd: "/repo",
                file: "index.ts",
            })
        ).ast;
        const result: CodeResult = await createGenerator(
            ast,
            resolvedGeneratorOptions({ sourceMap: { output: "inline" } }),
        ).generate();

        expect(result.map).toBeNull();
        expect(countSourceMappingComments(result.code)).toBe(1);
        expect(
            result.code.startsWith(
                `const a = 1;\n${SOURCE_MAPPING_COMMENT}data:application/json;base64,`,
            ),
        ).toBe(true);
    });

    it("keeps the map and appends the configured URL for output external", async (): Promise<void> => {
        const ast: Ast = (
            await parse("const a = 1;", {
                cwd: "/repo",
                file: "index.ts",
            })
        ).ast;
        const result: CodeResult = await createGenerator(
            ast,
            resolvedGeneratorOptions({
                sourceMap: {
                    output: {
                        mode: "external",
                        url: "dist/index.js.map",
                    },
                },
            }),
        ).generate();

        expect(result.code).toBe(
            `const a = 1;\n${SOURCE_MAPPING_COMMENT}dist/index.js.map\n`,
        );
        expect(countSourceMappingComments(result.code)).toBe(1);
        expect(result.map?.sources).toEqual(["index.ts"]);
        expect(firstMappingSegment(result.map?.mappings ?? "")).toEqual({
            dstColumn: 0,
            sourceIndex: 0,
            sourceLine: 0,
            sourceColumn: 0,
        });
    });

    it("uses file.map when output external has no url", async (): Promise<void> => {
        const ast: Ast = (
            await parse("const a = 1;", {
                cwd: "/repo",
                file: "index.ts",
            })
        ).ast;
        const result: CodeResult = await createGenerator(
            ast,
            resolvedGeneratorOptions({ sourceMap: { output: "external" } }),
        ).generate();

        expect(result.code).toBe(
            `const a = 1;\n${SOURCE_MAPPING_COMMENT}index.ts.map\n`,
        );
        expect(countSourceMappingComments(result.code)).toBe(1);
        expect(result.map?.sources).toEqual(["index.ts"]);
    });

    it("keeps the map and appends exactly one comment for output both", async (): Promise<void> => {
        const ast: Ast = (
            await parse("const a = 1;", {
                cwd: "/repo",
                file: "index.ts",
            })
        ).ast;
        const result: CodeResult = await createGenerator(
            ast,
            resolvedGeneratorOptions({ sourceMap: { output: "both" } }),
        ).generate();

        expect(result.map?.sources).toEqual(["index.ts"]);
        expect(countSourceMappingComments(result.code)).toBe(1);
        expect(
            result.code.startsWith(
                `const a = 1;\n${SOURCE_MAPPING_COMMENT}data:application/json;base64,`,
            ),
        ).toBe(true);
    });
});

describe("empty file option", (): void => {
    it("treats an empty file as the index.ts default", async (): Promise<void> => {
        const result: CodeResult = await compile("const a: number = 1;", {
            file: "",
        });

        expect(result.code).toBe("const a: number = 1;");
        expect(result.map?.sources).toEqual(["index.ts"]);
    });

    it("uses index.ts.map for an empty file in external output", async (): Promise<void> => {
        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "",
            sourceMap: { output: "external" },
        });

        expect(result.code).toBe(
            `const a = 1;\n${SOURCE_MAPPING_COMMENT}index.ts.map\n`,
        );
        expect(result.map?.sources).toEqual(["index.ts"]);
    });

    it.skipIf(isWasiWindows)(
        "reads index.ts.map for an empty file in external input mode",
        async (): Promise<void> => {
            const directory: string = await tempDir();
            const externalMap: SourceMap = buildMap({
                file: "external.ts",
                mappings: "AAMA",
                names: [],
                sources: ["external.ts"],
                sourcesContent: ["const a = 1;"],
            });

            await Fsp.writeFile(
                Path.join(directory, "index.ts.map"),
                JSON.stringify(externalMap),
                "utf-8",
            );

            const result: CodeResult = await compile("const a = 1;", {
                cwd: directory,
                file: "",
                sourceMap: { input: "external" },
            });

            expect(result.map?.sources).toEqual(["external.ts"]);
        },
    );

    it("treats an empty file as the index.ts default in compileFromAst", async (): Promise<void> => {
        const ast: Ast = (
            await parse("const a = 1;", {
                cwd: "/repo",
                file: "index.ts",
            })
        ).ast;

        const result: CodeResult = await compileFromAst(ast, {
            cwd: "/repo",
            file: "",
            sourceMap: { output: "external" },
        });

        expect(result.code).toBe(
            `const a = 1;\n${SOURCE_MAPPING_COMMENT}index.ts.map\n`,
        );
        expect(result.map?.sources).toEqual(["index.ts"]);
    });

    it("treats an empty file as the index.ts default in generate", async (): Promise<void> => {
        const ast: Ast = (
            await parse("const a = 1;", {
                cwd: "/repo",
                file: "index.ts",
            })
        ).ast;

        const result: CodeResult = await generate(ast, {
            cwd: "/repo",
            file: "",
            sourceMap: { output: "external" },
        });

        expect(result.code).toBe(
            `const a = 1;\n${SOURCE_MAPPING_COMMENT}index.ts.map\n`,
        );
        expect(result.map?.sources).toEqual(["index.ts"]);
    });

    it("lets an explicit url win over the index.ts default for an empty file", async (): Promise<void> => {
        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "",
            sourceMap: {
                output: {
                    mode: "external",
                    url: "custom.js.map",
                },
            },
        });

        expect(result.code).toBe(
            `const a = 1;\n${SOURCE_MAPPING_COMMENT}custom.js.map\n`,
        );
        expect(result.map?.sources).toEqual(["index.ts"]);
    });
});

describe("output path edges: compile", (): void => {
    it("derives the external map URL from the resolved file when output.file is unset", async (): Promise<void> => {
        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "src/index.ts",
            sourceMap: { output: "external" },
        });

        expectExternalMap(result, {
            source: "src/index.ts",
            url: "src/index.ts.map",
        });
    });

    it("falls back to file when output.file is empty", async (): Promise<void> => {
        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "src/index.ts",
            output: { file: "" },
            sourceMap: { output: "external" },
        });

        expectExternalMap(result, {
            source: "src/index.ts",
            url: "src/index.ts.map",
        });
    });

    it("uses index.ts.map when file and output.file are both empty", async (): Promise<void> => {
        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "",
            output: { file: "" },
            sourceMap: { output: "external" },
        });

        expectExternalMap(result, {
            source: "index.ts",
            url: "index.ts.map",
        });
    });

    it("lets an explicit url win over the output.file default", async (): Promise<void> => {
        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "src/index.ts",
            output: { file: "dist/index.js" },
            sourceMap: {
                output: {
                    mode: "external",
                    url: "custom.js.map",
                },
            },
        });

        expectExternalMap(result, {
            source: "src/index.ts",
            url: "custom.js.map",
        });
    });

    it("falls back to output.file when sourceMap.output.url is empty", async (): Promise<void> => {
        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "src/index.ts",
            output: { file: "dist/index.js" },
            sourceMap: {
                output: {
                    mode: "external",
                    url: "",
                },
            },
        });

        expectExternalMap(result, {
            source: "src/index.ts",
            url: "dist/index.js.map",
        });
    });
});

describe("output path edges: compileFromAst", (): void => {
    it("derives the external map URL from the resolved file when output.file is unset", async (): Promise<void> => {
        const ast: Ast = await plainAst();

        const result: CodeResult = await compileFromAst(ast, {
            cwd: "/repo",
            file: "src/index.ts",
            sourceMap: { output: "external" },
        });

        expectExternalMap(result, {
            source: "src/index.ts",
            url: "src/index.ts.map",
        });
    });

    it("falls back to file when output.file is empty", async (): Promise<void> => {
        const ast: Ast = await plainAst();

        const result: CodeResult = await compileFromAst(ast, {
            cwd: "/repo",
            file: "src/index.ts",
            output: { file: "" },
            sourceMap: { output: "external" },
        });

        expectExternalMap(result, {
            source: "src/index.ts",
            url: "src/index.ts.map",
        });
    });

    it("uses index.ts.map when file and output.file are both empty", async (): Promise<void> => {
        const ast: Ast = await plainAst();

        const result: CodeResult = await compileFromAst(ast, {
            cwd: "/repo",
            file: "",
            output: { file: "" },
            sourceMap: { output: "external" },
        });

        expectExternalMap(result, {
            source: "index.ts",
            url: "index.ts.map",
        });
    });

    it("lets an explicit url win over the output.file default", async (): Promise<void> => {
        const ast: Ast = await plainAst();

        const result: CodeResult = await compileFromAst(ast, {
            cwd: "/repo",
            file: "src/index.ts",
            output: { file: "dist/index.js" },
            sourceMap: {
                output: {
                    mode: "external",
                    url: "custom.js.map",
                },
            },
        });

        expectExternalMap(result, {
            source: "src/index.ts",
            url: "custom.js.map",
        });
    });

    it("falls back to output.file when sourceMap.output.url is empty", async (): Promise<void> => {
        const ast: Ast = await plainAst();

        const result: CodeResult = await compileFromAst(ast, {
            cwd: "/repo",
            file: "src/index.ts",
            output: { file: "dist/index.js" },
            sourceMap: {
                output: {
                    mode: "external",
                    url: "",
                },
            },
        });

        expectExternalMap(result, {
            source: "src/index.ts",
            url: "dist/index.js.map",
        });
    });
});

describe("output path edges: generate", (): void => {
    it("derives the external map URL from the resolved file when output.file is unset", async (): Promise<void> => {
        const ast: Ast = await plainAst();

        const result: CodeResult = await generate(ast, {
            cwd: "/repo",
            file: "src/index.ts",
            sourceMap: { output: "external" },
        });

        expectExternalMap(result, {
            source: "src/index.ts",
            url: "src/index.ts.map",
        });
    });

    it("falls back to file when output.file is empty", async (): Promise<void> => {
        const ast: Ast = await plainAst();

        const result: CodeResult = await generate(ast, {
            cwd: "/repo",
            file: "src/index.ts",
            output: { file: "" },
            sourceMap: { output: "external" },
        });

        expectExternalMap(result, {
            source: "src/index.ts",
            url: "src/index.ts.map",
        });
    });

    it("uses index.ts.map when file and output.file are both empty", async (): Promise<void> => {
        const ast: Ast = await plainAst();

        const result: CodeResult = await generate(ast, {
            cwd: "/repo",
            file: "",
            output: { file: "" },
            sourceMap: { output: "external" },
        });

        expectExternalMap(result, {
            source: "index.ts",
            url: "index.ts.map",
        });
    });

    it("falls back to file when sourceMap.output.url is empty", async (): Promise<void> => {
        const ast: Ast = await plainAst();

        const result: CodeResult = await generate(ast, {
            cwd: "/repo",
            file: "src/index.ts",
            sourceMap: {
                output: {
                    mode: "external",
                    url: "",
                },
            },
        });

        expectExternalMap(result, {
            source: "src/index.ts",
            url: "src/index.ts.map",
        });
    });
});

describe("output path edges: createGenerator", (): void => {
    it("derives the external map URL from file", async (): Promise<void> => {
        const ast: Ast = await plainAst();

        const result: CodeResult = await createGenerator(
            ast,
            resolvedGeneratorOptions({
                file: "src/index.ts",
                sourceMap: { output: "external" },
            }),
        ).generate();

        expectExternalMap(result, {
            source: "src/index.ts",
            url: "src/index.ts.map",
        });
    });

    it("uses index.ts.map when file is empty", async (): Promise<void> => {
        const ast: Ast = await plainAst();

        const result: CodeResult = await createGenerator(
            ast,
            resolvedGeneratorOptions({
                file: "",
                sourceMap: { output: "external" },
            }),
        ).generate();

        expectExternalMap(result, {
            source: "index.ts",
            url: "index.ts.map",
        });
    });

    it("falls back to file when sourceMap.output.url is empty", async (): Promise<void> => {
        const ast: Ast = await plainAst();

        const result: CodeResult = await createGenerator(
            ast,
            resolvedGeneratorOptions({
                file: "src/index.ts",
                sourceMap: {
                    output: {
                        mode: "external",
                        url: "",
                    },
                },
            }),
        ).generate();

        expectExternalMap(result, {
            source: "src/index.ts",
            url: "src/index.ts.map",
        });
    });
});
