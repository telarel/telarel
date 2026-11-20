import type {
    AstResult,
    CodeResult,
    Comment,
    DeepReadonly,
    ParseArgs,
    PluginContext,
    TransformArgs,
    TransformResult,
} from "telarel";

import { compile, generate, parse } from "telarel";
import { describe, expect, it } from "vitest";

import {
    appendInlineMap,
    buildMap,
    countSourceMappingComments,
    SOURCE_MAPPING_COMMENT,
} from "#/functions/sourcemap";

const upstreamMap = (): ReturnType<typeof buildMap> =>
    buildMap({
        file: "upstream.ts",
        mappings: "AASA",
        names: [],
        sources: ["upstream.ts"],
        sourcesContent: ["const a = 1;"],
    });

describe("source map comment", (): void => {
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
});
