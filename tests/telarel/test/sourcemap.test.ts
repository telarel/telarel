import type {
    CodeResult,
    CommonPluginContext,
    DeepReadonly,
    OptionsArgs,
    PluginContext,
    SourceMap,
    SourceMapOptions,
    StartArgs,
    TransformArgs,
    TransformResult,
} from "telarel";

import type { ProgramFixture } from "#/functions/ast";

import { compile } from "telarel";
import { describe, expect, it } from "vitest";

import { renameRootIdentifier } from "#/functions/ast";
import { buildMap } from "#/functions/sourcemap";

describe("sourcemap", (): void => {
    it("emits a v3 codegen map with observed exact fields", async (): Promise<void> => {
        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [],
        });

        expect(typeof result.map).toBe("object");
        expect(result.map?.version).toBe(3);
        // Parse always runs, so the map is the real codegen map: it names the
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

        // Codegen prints the program, so the leading BOM is dropped from the
        // output; the WASI code path also strips a leading U+FEFF.
        expect(result.code.replace(/^\uFEFF/gu, "")).toBe(
            'const µ = "日本語";',
        );
        expect(result.map?.version).toBe(3);
        expect(result.map?.sources).toEqual(["index.ts"]);
        expect(result.map?.mappings).toBe("AAAC,MAAM,IAAI");
        // `sourcesContent` mirrors the original text, but the leading BOM's
        // survival is runtime-dependent (the WASI TextDecoder strips a
        // leading U+FEFF while the native binding preserves it) — pin only
        // the runtime-independent remainder.
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
        // Codegen mappings: one statement per generated line, each naming the
        // identifier, the literal, and the end of the statement.
        expect(result.map?.mappings).toBe("AAAA,MAAM,IAAI;AACV,MAAM,IAAI");
    });

    it("normalizes CRLF in code but keeps it in sourcesContent", async (): Promise<void> => {
        const result: CodeResult = await compile(
            "const a = 1;\r\nconst b = 2;",
            {
                cwd: "/repo",
                file: "index.ts",
            },
        );

        // Codegen prints the program, so CRLF line endings are normalized to
        // LF in the output; `sourcesContent` keeps the original text.
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

        // The trailing `\r\n` terminator is stripped by codegen, and the map
        // still covers the single statement.
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
        // Codegen records the source identifier name the renamed output maps
        // back to; the source text still reads `a`.
        expect(result.map?.names).toEqual(["a"]);
        expect(result.map?.mappings.length).toBeGreaterThan(0);
        // A changed AST must not produce byte-identical mappings: the
        // generated identifier is longer, so the later segments' column deltas
        // shift.
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
});
