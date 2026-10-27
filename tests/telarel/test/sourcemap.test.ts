import type {
    AstResult,
    CodeResult,
    Comment,
    CommonPluginContext,
    DeepReadonly,
    OptionsArgs,
    ParseArgs,
    PluginContext,
    SourceMap,
    SourceMapOptions,
    StartArgs,
    TransformArgs,
    TransformResult,
} from "telarel";

import type { ProgramFixture } from "#/functions/ast";

import * as Fsp from "node:fs/promises";
import * as Path from "node:path";

import { compile, generate, parse } from "telarel";
import { afterEach, describe, expect, it } from "vitest";

import { renameRootIdentifier } from "#/functions/ast";
import {
    appendInlineMap,
    buildMap,
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

const countSourceMappingComments = (code: string): number =>
    code.split(SOURCE_MAPPING_COMMENT).length - 1;

describe("sourcemap", (): void => {
    it("emits a v3 codegen map with observed exact fields", async (): Promise<void> => {
        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [],
        });

        expect(typeof result.map).toBe("object");
        expect(result.map?.version).toBe(3);
        // Parse always runs, so the map is the real codegen map. It names the
        // input file as its single source and carries the encoded mappings.
        expect(result.map?.file).toBe(void 0);
        expect(result.map?.sourceRoot).toBe(void 0);
        expect(result.map?.ignoreList).toBe(void 0);
        expect(result.map?.sources).toEqual(["index.ts"]);
        expect(result.map?.sourcesContent).toEqual(["const a = 1;"]);
        expect(result.map?.names).toEqual([]);
        expect(result.map?.mappings.length).toBeGreaterThan(0);
    });

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
        expect(result.map?.file).toBe(void 0);
        expect(result.map?.sources).toEqual(["index.ts"]);
        expect(result.map?.mappings.length).toBeGreaterThan(0);
    });

    it("compiles empty code to empty code and an empty codegen map", async (): Promise<void> => {
        const result: CodeResult = await compile("", {
            cwd: "/repo",
            file: "index.ts",
        });

        expect(result.code).toBe("");
        expect(result.map?.mappings).toBe("");
    });

    it("round-trips BOM and non-ASCII code", async (): Promise<void> => {
        const result: CodeResult = await compile('\uFEFFconst µ = "日本語";', {
            cwd: "/repo",
            file: "index.ts",
        });

        // Codegen prints the program, so it drops the leading BOM from the
        // output. The WASI code path also strips a leading U+FEFF.
        expect(result.code.replace(/^\uFEFF/gu, "")).toBe(
            'const µ = "日本語";',
        );
        expect(result.map?.version).toBe(3);
        expect(result.map?.sources).toEqual(["index.ts"]);
        expect(result.map?.mappings).toBe("AAAC,MAAM,IAAI");
        // `sourcesContent` mirrors the original text, but whether the leading
        // BOM survives depends on the runtime. The WASI TextDecoder strips a
        // leading U+FEFF, and the native binding preserves it. Pin only the
        // runtime-independent remainder.
        expect(result.map?.sourcesContent).not.toBe(void 0);
        expect(result.map?.sourcesContent?.[0]?.replace(/^\uFEFF/gu, "")).toBe(
            'const µ = "日本語";',
        );
    });

    it("emits mappings for non-ASCII code through the transform path", async (): Promise<void> => {
        const result: CodeResult = await compile('const µ = "日本語";', {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "pass-through",
                    transform: (): void => void 0,
                },
            ],
        });

        expect(result.map?.version).toBe(3);
        expect(result.map?.mappings.length).toBeGreaterThan(0);
    });

    it("pins mappings across multiple lines", async (): Promise<void> => {
        const result: CodeResult = await compile("const a = 1;\nconst b = 2;", {
            cwd: "/repo",
            file: "index.ts",
        });

        expect(result.code).toBe("const a = 1;\nconst b = 2;");
        expect(result.map?.version).toBe(3);
        // Codegen emits one statement per generated line. The three segments on
        // each line mark the start of the statement, the identifier, and the
        // literal.
        expect(result.map?.mappings).toBe("AAAA,MAAM,IAAI;AACV,MAAM,IAAI");
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

    it("normalizes CRLF in code but keeps it in sourcesContent", async (): Promise<void> => {
        const result: CodeResult = await compile(
            "const a = 1;\r\nconst b = 2;",
            {
                cwd: "/repo",
                file: "index.ts",
            },
        );

        // Codegen prints the program, so it normalizes CRLF line endings to LF
        // in the output. `sourcesContent` keeps the original text.
        expect(result.code).toBe("const a = 1;\nconst b = 2;");
        expect(result.map?.version).toBe(3);
        expect(result.map?.sourcesContent).toEqual([
            "const a = 1;\r\nconst b = 2;",
        ]);
        expect(result.map?.mappings).toBe("AAAA,MAAM,IAAI;AACV,MAAM,IAAI");
    });

    it("strips a trailing CRLF from code and emits codegen mappings", async (): Promise<void> => {
        const result: CodeResult = await compile("const a = 1;\r\n", {
            cwd: "/repo",
            file: "index.ts",
        });

        // Codegen strips the trailing `\r\n` terminator, and the map still
        // covers the single statement.
        expect(result.code).toBe("const a = 1;");
        expect(result.map?.version).toBe(3);
        expect(result.map?.mappings).toBe("AAAA,MAAM,IAAI");
    });

    it("uses the given file name as the single source", async (): Promise<void> => {
        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "entry.tsx",
        });

        expect(result.map?.sources).toEqual(["entry.tsx"]);
        expect(result.map?.mappings).toBe("AAAA,MAAM,IAAI");
    });

    it("keeps mappings identical after a pass-through transform", async (): Promise<void> => {
        const baseline: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "baseline-pass-through",
                    transform: (): void => void 0,
                },
            ],
        });

        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "pass-through",
                    transform: (): void => void 0,
                },
            ],
        });

        expect(result.map?.mappings).toBe(baseline.map?.mappings);
        expect(result.map?.sources).toEqual(baseline.map?.sources);
    });

    it("shifts mappings after a length-changing transform", async (): Promise<void> => {
        const baseline: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [],
        });

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
                        const program: ProgramFixture = args.ast
                            .program as unknown as ProgramFixture;
                        renameRootIdentifier({ program, name: "renamed-long" });

                        return { ast: args.ast };
                    },
                },
            ],
        });

        expect(result.code).toBe("const renamed-long = 1;");
        expect(result.map?.version).toBe(3);
        expect(result.map?.sources).toEqual(["index.ts"]);
        // Codegen records the source identifier name that the renamed output
        // maps back to. The source text still reads `a`.
        expect(result.map?.names).toEqual(["a"]);
        expect(result.map?.mappings.length).toBeGreaterThan(0);
        // A changed AST must not produce byte-identical mappings. The generated
        // identifier is longer, so the column deltas of later segments shift.
        expect(result.map?.mappings).not.toBe(baseline.map?.mappings);
    });

    it("pins exact mappings across three lines", async (): Promise<void> => {
        const result: CodeResult = await compile(
            "const a = 1;\nconst b = 2;\nconst c = 3;",
            {
                cwd: "/repo",
                file: "index.ts",
            },
        );

        expect(result.code).toBe("const a = 1;\nconst b = 2;\nconst c = 3;");
        expect(result.map?.version).toBe(3);
        expect(result.map?.mappings).toBe(
            "AAAA,MAAM,IAAI;AACV,MAAM,IAAI;AACV,MAAM,IAAI",
        );
    });

    it("removes the consumed comment when no output map is requested", async (): Promise<void> => {
        const code: string = appendInlineMap("const a = 1;", upstreamMap());

        const result: CodeResult = await compile(code, {
            cwd: "/repo",
            file: "index.ts",
            sourceMap: { input: "inline" },
        });

        expect(result.code).toBe("const a = 1;");
        expect(countSourceMappingComments(result.code)).toBe(0);
        expect(result.map?.sources).toEqual(["upstream.ts"]);
    });

    it("handles non-ASCII source before the consumed comment", async (): Promise<void> => {
        const source: string = 'const µ = "日本語";';
        const code: string = appendInlineMap(source, upstreamMap());

        const result: CodeResult = await compile(code, {
            cwd: "/repo",
            file: "index.ts",
            sourceMap: { input: "inline" },
        });

        expect(result.code).toBe(source);
        expect(countSourceMappingComments(result.code)).toBe(0);
    });

    it("emits exactly one inline comment when output is inline", async (): Promise<void> => {
        const code: string = appendInlineMap("const a = 1;", upstreamMap());

        const result: CodeResult = await compile(code, {
            cwd: "/repo",
            file: "index.ts",
            sourceMap: {
                input: "inline",
                output: "inline",
            },
        });

        expect(result.map).toBeNull();
        expect(countSourceMappingComments(result.code)).toBe(1);
        expect(
            result.code.startsWith(
                `const a = 1;\n${SOURCE_MAPPING_COMMENT}data:application/json;base64,`,
            ),
        ).toBe(true);
    });

    it("emits exactly one external comment when output is external", async (): Promise<void> => {
        const code: string = appendInlineMap("const a = 1;", upstreamMap());

        const result: CodeResult = await compile(code, {
            cwd: "/repo",
            file: "index.ts",
            output: { file: "dist/index.js" },
            sourceMap: {
                input: "inline",
                output: "external",
            },
        });

        expect(result.code).toBe(
            `const a = 1;\n${SOURCE_MAPPING_COMMENT}dist/index.js.map\n`,
        );
        expect(countSourceMappingComments(result.code)).toBe(1);
        expect(result.map?.sources).toEqual(["upstream.ts"]);
    });

    it("removes the consumed comment in auto mode", async (): Promise<void> => {
        const code: string = appendInlineMap("const a = 1;", upstreamMap());

        const result: CodeResult = await compile(code, {
            cwd: "/repo",
            file: "index.ts",
            sourceMap: { input: "auto" },
        });

        expect(result.code).toBe("const a = 1;");
        expect(countSourceMappingComments(result.code)).toBe(0);
        expect(result.map?.sources).toEqual(["upstream.ts"]);
    });

    it("keeps comments that resemble source-map comments", async (): Promise<void> => {
        const code: string = `// see sourceMappingURL=docs\n${appendInlineMap(
            "const a = 1;",
            upstreamMap(),
        )}`;

        const result: CodeResult = await compile(code, {
            cwd: "/repo",
            file: "index.ts",
            sourceMap: { input: "inline" },
        });

        expect(result.code).toContain("// see sourceMappingURL=docs");
        expect(result.code).not.toContain(SOURCE_MAPPING_COMMENT);
        expect(result.code).toBe("// see sourceMappingURL=docs\nconst a = 1;");
    });

    it("does not restore the consumed comment after a transform pass-through", async (): Promise<void> => {
        const code: string = appendInlineMap("const a = 1;", upstreamMap());

        const result: CodeResult = await compile(code, {
            cwd: "/repo",
            file: "index.ts",
            sourceMap: { input: "inline" },
            plugins: [
                {
                    name: "pass-through",
                    transform: (
                        _ctx: PluginContext,
                        args: TransformArgs,
                    ): TransformResult => ({ ast: args.ast }),
                },
            ],
        });

        expect(result.code).toBe("const a = 1;");
        expect(countSourceMappingComments(result.code)).toBe(0);
    });

    it("removes the comment after the pipeline attaches the map to a winning parse hook's AST", async (): Promise<void> => {
        const code: string = appendInlineMap("const a = 1;", upstreamMap());

        const parsed: AstResult = await parse(code, {
            cwd: "/repo",
            file: "index.ts",
        });

        expect(parsed.ast.source).toContain(SOURCE_MAPPING_COMMENT);

        const result: CodeResult = await compile(code, {
            cwd: "/repo",
            file: "index.ts",
            sourceMap: { input: "inline" },
            plugins: [
                {
                    name: "parse-without-map",
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
    });

    it("keeps the inline comment when the AST already carries its own input map", async (): Promise<void> => {
        const code: string = appendInlineMap("const a = 1;", upstreamMap());
        const ownMap: SourceMap = buildMap({
            file: "own.ts",
            mappings: "AASA",
            names: [],
            sources: ["own.ts"],
            sourcesContent: ["const a = 1;"],
        });

        const parsed: AstResult = await parse(code, {
            cwd: "/repo",
            file: "index.ts",
            inputSourceMap: ownMap,
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

        expect(result.code).toContain(SOURCE_MAPPING_COMMENT);
        expect(countSourceMappingComments(result.code)).toBe(1);
        expect(result.map?.sources).toEqual(["own.ts"]);
    });

    it("removes the comment from a parsed AST but keeps the source text", async (): Promise<void> => {
        const code: string = appendInlineMap("const a = 1;", upstreamMap());

        const parsed: AstResult = await parse(code, {
            cwd: "/repo",
            file: "index.ts",
            inputSourceMap: "inline",
        });

        expect(parsed.ast.map?.sources).toEqual(["upstream.ts"]);
        expect(parsed.ast.source).toContain(SOURCE_MAPPING_COMMENT);
        expect(
            (parsed.ast.program.comments ?? []).some(
                (comment: DeepReadonly<Comment>): boolean =>
                    parsed.ast.source
                        .slice(comment.start, comment.end)
                        .includes("sourceMappingURL"),
            ),
        ).toBe(false);

        const generated: CodeResult = await generate(parsed.ast, {
            cwd: "/repo",
            file: "index.ts",
        });

        expect(generated.code).toBe("const a = 1;");
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
