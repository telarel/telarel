import type {
    AstResult,
    CodeResult,
    Comment,
    DeepReadonly,
    Generator,
    OxcProgram,
    ParseArgs,
    PluginContext,
    Program,
    TransformArgs,
    TransformResult,
} from "telarel";

import {
    Ast,
    compile,
    compileFromAst,
    createGenerator,
    generate,
    parse,
    rewrite,
} from "telarel";
import { describe, expect, it } from "vitest";

import { resolvedGeneratorOptions } from "#/functions/options";

const SOURCE: string = "// lead\nconst µ = 1; /* trail */\n// tail";

const COMPILED_COMMENTS: ReadonlyArray<string> = [
    "// lead",
    "/* trail */",
    "// tail",
];

const COMMENT_VALUES: ReadonlyArray<string> = [" lead", " trail ", " tail"];

type InnerCommentCase = {
    source: string;
    comment: string;
};

const INNER_COMMENT_CASES: ReadonlyArray<InnerCommentCase> = [
    { source: "function f(/* c */ a) {}", comment: "/* c */" },
    { source: "const x = /* c */ 1;", comment: "/* c */" },
    { source: "const x = [/* c */ 1];", comment: "/* c */" },
    { source: "const { /* c */ a } = x;", comment: "/* c */" },
    { source: "const [/* c */ a] = x;", comment: "/* c */" },
    { source: 'import { /* c */ x } from "y";', comment: "/* c */" },
    { source: "function f(/** jsdoc */ a) {}", comment: "/** jsdoc */" },
];

const copyProgram = (program: DeepReadonly<Program>): Program =>
    structuredClone(program) as unknown as Program;

const commentValues = (program: DeepReadonly<Program>): Array<string> =>
    (program.comments ?? []).map(
        (comment: DeepReadonly<Comment>): string => comment.value,
    );

const expectPreserved = (code: string): void => {
    for (const comment of COMPILED_COMMENTS) {
        expect(code).toContain(comment);
    }
};

const parsedSource = async (source: string): Promise<AstResult> =>
    await parse(source, { file: "index.js" });

describe("comment preservation", (): void => {
    it("parse exposes comments sorted by start with UTF-16 spans", async (): Promise<void> => {
        const source: string =
            'const smile = "😀";\n// first\n/* second */\nconst b = 2;';
        const parsed: AstResult = await parsedSource(source);
        const comments: ReadonlyArray<DeepReadonly<Comment>> =
            parsed.ast.program.comments ?? [];
        const firstStart: number = source.indexOf("// first");
        const secondStart: number = source.indexOf("/* second */");
        const starts: Array<number> = comments.map(
            (comment: DeepReadonly<Comment>): number => comment.start,
        );

        expect(comments).toHaveLength(2);
        expect(starts).toEqual(
            starts.toSorted((left, right): number => left - right),
        );
        expect(comments[0]?.type).toBe("Line");
        expect(comments[0]?.value).toBe(" first");
        expect(comments[0]?.start).toBe(firstStart);
        expect(comments[0]?.end).toBe(firstStart + "// first".length);
        expect(source.slice(comments[0]?.start, comments[0]?.end)).toBe(
            "// first",
        );
        expect(comments[1]?.type).toBe("Block");
        expect(comments[1]?.value).toBe(" second ");
        expect(comments[1]?.start).toBe(secondStart);
        expect(comments[1]?.end).toBe(secondStart + "/* second */".length);
        expect(source.slice(comments[1]?.start, comments[1]?.end)).toBe(
            "/* second */",
        );
    });

    it("preserves comments through generate(ast)", async (): Promise<void> => {
        const parsed: AstResult = await parsedSource(SOURCE);
        const result: CodeResult = await generate(parsed.ast, {
            file: "index.js",
        });

        expectPreserved(result.code);
        expect(result.code).toContain("const µ = 1;");
    });

    it("preserves comments through generate(program, code)", async (): Promise<void> => {
        const parsed: AstResult = await parsedSource(SOURCE);
        const program: Program = copyProgram(parsed.ast.program);
        const result: CodeResult = await generate(program, parsed.ast.source, {
            file: "index.js",
        });

        expectPreserved(result.code);
        expect(result.code).toContain("const µ = 1;");
    });

    it("preserves comments through rewrite", async (): Promise<void> => {
        const parsed: AstResult = await parsedSource(SOURCE);
        const result: AstResult = await rewrite(parsed.ast, {
            file: "index.js",
        });
        const generated: CodeResult = await generate(result.ast, {
            file: "index.js",
        });

        expect(result.ast.source).toBe(SOURCE);
        expect(commentValues(result.ast.program)).toEqual(COMMENT_VALUES);
        expectPreserved(generated.code);
    });

    it("preserves comments through compileFromAst", async (): Promise<void> => {
        const parsed: AstResult = await parsedSource(SOURCE);
        const result: CodeResult = await compileFromAst(parsed.ast, {
            file: "index.js",
        });

        expectPreserved(result.code);
        expect(result.code).toContain("const µ = 1;");
    });

    it("preserves comments through createGenerator", async (): Promise<void> => {
        const parsed: AstResult = await parsedSource(SOURCE);
        const generator: Generator = createGenerator(
            parsed.ast,
            resolvedGeneratorOptions({ file: "index.js" }),
        );
        const result: CodeResult = await generator.generate();

        expectPreserved(result.code);
        expect(result.code).toContain("const µ = 1;");
    });

    it("preserves comments in inner positions through every AST path", async (): Promise<void> => {
        for (const { source, comment } of INNER_COMMENT_CASES) {
            const parsed: AstResult = await parsedSource(source);
            const generated: CodeResult = await generate(parsed.ast, {
                file: "index.js",
            });

            expect(generated.code).toContain(comment);

            const program: Program = copyProgram(parsed.ast.program);
            const generatedProgram: CodeResult = await generate(
                program,
                parsed.ast.source,
                { file: "index.js" },
            );

            expect(generatedProgram.code).toContain(comment);

            const rewritten: AstResult = await rewrite(parsed.ast, {
                file: "index.js",
            });
            const generatedRewrite: CodeResult = await generate(rewritten.ast, {
                file: "index.js",
            });

            expect(generatedRewrite.code).toContain(comment);

            const compiled: CodeResult = await compileFromAst(parsed.ast, {
                file: "index.js",
            });

            expect(compiled.code).toContain(comment);

            const generator: Generator = createGenerator(
                parsed.ast,
                resolvedGeneratorOptions({ file: "index.js" }),
            );
            const generatedByGenerator: CodeResult = await generator.generate();

            expect(generatedByGenerator.code).toContain(comment);

            const transformed: CodeResult = await compile(source, {
                cwd: "/repo",
                file: "index.js",
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

            expect(transformed.code).toContain(comment);

            const replaced: CodeResult = await compile(source, {
                cwd: "/repo",
                file: "index.js",
                plugins: [
                    {
                        name: "parse-replacement",
                        parse: (
                            _ctx: PluginContext,
                            _args: ParseArgs,
                        ): AstResult => ({ ast: parsed.ast }),
                    },
                ],
            });

            expect(replaced.code).toContain(comment);
        }
    });

    it("preserves a trailing comment through a transform pass-through", async (): Promise<void> => {
        const result: CodeResult = await compile("const x = 1; // trail", {
            cwd: "/repo",
            file: "index.js",
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

        expect(result.code).toContain("// trail");
    });

    it("preserves a pure annotation through a transform pass-through", async (): Promise<void> => {
        const result: CodeResult = await compile("/*#__PURE__*/ f();", {
            cwd: "/repo",
            file: "index.js",
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

        expect(result.code).toContain("/*#__PURE__*/");
    });

    it("preserves a pure annotation's text through a JSON rebuild", async (): Promise<void> => {
        const parsed: AstResult = await parsedSource("/*#__PURE__*/ f();");
        const rewritten: AstResult = await rewrite(parsed.ast, {
            file: "index.js",
        });

        expect(rewritten.ast.program.comments?.[0]?.content).toBe("none");

        const generated: CodeResult = await generate(rewritten.ast, {
            file: "index.js",
        });

        expect(generated.code).toContain("/*#__PURE__*/");
    });

    it("accepts an OxcProgram across entry points", async (): Promise<void> => {
        const parsed: AstResult = await parsedSource("// keep\nconst a = 1;");
        const program: OxcProgram = copyProgram(parsed.ast.program);
        const ast: Ast = new Ast({ program, source: parsed.ast.source });
        const generated: CodeResult = await generate(
            program,
            parsed.ast.source,
            { file: "index.js" },
        );
        const rewritten: AstResult = await rewrite(program, parsed.ast.source, {
            file: "index.js",
        });
        const compiled: CodeResult = await compileFromAst(
            program,
            parsed.ast.source,
            { file: "index.js" },
        );

        expect(ast.program.type).toBe("Program");
        expect(generated.code).toContain("// keep");
        expect(rewritten.ast.program.type).toBe("Program");
        expect(compiled.code).toContain("// keep");
    });
});
