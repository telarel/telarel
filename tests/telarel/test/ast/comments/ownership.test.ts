import type {
    AstResult,
    CodeResult,
    Comment,
    DeepReadonly,
    ParseArgs,
    PluginContext,
    Program,
    SourceMap,
    TransformArgs,
    TransformResult,
} from "telarel";

import {
    Ast,
    compile,
    compileFromAst,
    generate,
    parse,
    rewrite,
} from "telarel";
import { describe, expect, it } from "vitest";

import {
    appendInlineMap,
    buildMap,
    SOURCE_MAPPING_COMMENT,
} from "#/functions/sourcemap";

const SOURCE: string = "// lead\nconst µ = 1; /* trail */\n// tail";

const copyProgram = (program: DeepReadonly<Program>): Program =>
    structuredClone(program) as unknown as Program;

const hasSourceMappingComment = (result: AstResult): boolean =>
    (result.ast.program.comments ?? []).some(
        (comment: DeepReadonly<Comment>): boolean =>
            result.ast.source
                .slice(comment.start, comment.end)
                .includes(SOURCE_MAPPING_COMMENT),
    );

const inlineMapSource = (): string => {
    const map: SourceMap = buildMap({
        file: "index.js",
        sources: ["index.js"],
        mappings: "AAAA",
    });

    return appendInlineMap("const a = 1;", map);
};

const parsedSource = async (source: string): Promise<AstResult> =>
    await parse(source, { file: "index.js" });

describe("comment ownership", (): void => {
    it("keeps a winning parse hook's comments", async (): Promise<void> => {
        const replacement: AstResult = await parsedSource(
            "// replacement\nconst b = 2;",
        );
        const result: CodeResult = await compile("// original\nconst a = 1;", {
            cwd: "/repo",
            file: "index.js",
            plugins: [
                {
                    name: "parse-replacement",
                    parse: (
                        _ctx: PluginContext,
                        _args: ParseArgs,
                    ): AstResult => ({ ast: replacement.ast }),
                },
            ],
        });

        expect(result.code).toContain("// replacement");
        expect(result.code).not.toContain("// original");
    });

    it("drops a winning parse hook's comments when it returns comments: []", async (): Promise<void> => {
        const replacement: AstResult = await parsedSource(
            "// replacement\nconst b = 2;",
        );
        const program: Program = copyProgram(replacement.ast.program);
        program.comments = [];
        const ast: Ast = new Ast({ program, source: replacement.ast.source });
        const result: CodeResult = await compile("// original\nconst a = 1;", {
            cwd: "/repo",
            file: "index.js",
            plugins: [
                {
                    name: "parse-empty-comments",
                    parse: (
                        _ctx: PluginContext,
                        _args: ParseArgs,
                    ): AstResult => ({ ast }),
                },
            ],
        });

        expect(result.code).not.toContain("// replacement");
        expect(result.code).not.toContain("// original");
    });

    it("keeps a transform replacement AST's comments", async (): Promise<void> => {
        const replacement: AstResult = await parsedSource(
            "// replacement\nconst b = 2;",
        );
        const result: CodeResult = await compile("// original\nconst a = 1;", {
            cwd: "/repo",
            file: "index.js",
            plugins: [
                {
                    name: "transform-replacement",
                    transform: (
                        _ctx: PluginContext,
                        _args: TransformArgs,
                    ): TransformResult => ({ ast: replacement.ast }),
                },
            ],
        });

        expect(result.code).toContain("// replacement");
        expect(result.code).not.toContain("// original");
    });

    it("drops the original comments when a transform returns an AST without comments", async (): Promise<void> => {
        const replacement: AstResult = await parsedSource(
            "// replacement\nconst b = 2;",
        );
        const program: Program = copyProgram(replacement.ast.program);
        delete program.comments;
        const ast: Ast = new Ast({ program, source: replacement.ast.source });
        const result: CodeResult = await compile("// original\nconst a = 1;", {
            cwd: "/repo",
            file: "index.js",
            plugins: [
                {
                    name: "transform-without-comments",
                    transform: (
                        _ctx: PluginContext,
                        _args: TransformArgs,
                    ): TransformResult => ({ ast }),
                },
            ],
        });

        expect(result.code).not.toContain("// replacement");
        expect(result.code).not.toContain("// original");
        expect(result.code).toBe("const b = 2;");
    });

    it("treats absent comments and comments: [] as no comments", async (): Promise<void> => {
        const parsed: AstResult = await parsedSource(SOURCE);
        const absent: Program = copyProgram(parsed.ast.program);
        delete absent.comments;
        const empty: Program = copyProgram(parsed.ast.program);
        empty.comments = [];
        const withoutComments: CodeResult = await generate(
            absent,
            parsed.ast.source,
            { file: "index.js" },
        );
        const withEmptyComments: CodeResult = await generate(
            empty,
            parsed.ast.source,
            { file: "index.js" },
        );

        expect(withoutComments.code).toBe("const µ = 1;");
        expect(withEmptyComments.code).toBe("const µ = 1;");
    });

    it("removes the consumed comment from a rewritten AST", async (): Promise<void> => {
        const source: string = inlineMapSource();
        const parsed: AstResult = await parsedSource(source);

        expect(hasSourceMappingComment(parsed)).toBe(true);

        const result: AstResult = await rewrite(parsed.ast, {
            file: "index.js",
            inputSourceMap: "inline",
        });

        expect(hasSourceMappingComment(result)).toBe(false);
        expect(result.ast.source).toContain(SOURCE_MAPPING_COMMENT);
    });

    it("removes the consumed comment before generate output", async (): Promise<void> => {
        const source: string = inlineMapSource();
        const parsed: AstResult = await parsedSource(source);
        const result: CodeResult = await generate(parsed.ast, {
            file: "index.js",
            sourceMap: { input: "inline" },
        });

        expect(result.code).toContain("const a = 1;");
        expect(result.code).not.toContain(SOURCE_MAPPING_COMMENT);
    });

    it("removes the consumed comment before compileFromAst output", async (): Promise<void> => {
        const source: string = inlineMapSource();
        const parsed: AstResult = await parsedSource(source);
        const result: CodeResult = await compileFromAst(parsed.ast, {
            file: "index.js",
            sourceMap: { input: "inline" },
        });

        expect(result.code).toContain("const a = 1;");
        expect(result.code).not.toContain(SOURCE_MAPPING_COMMENT);
    });
});
