import type {
    AstResult,
    CodeResult,
    Comment,
    DeepReadonly,
    Program,
} from "telarel";

import { generate, parse, rewrite } from "telarel";
import { describe, expect, it } from "vitest";

const copyProgram = (program: DeepReadonly<Program>): Program =>
    structuredClone(program) as unknown as Program;

const parsedSource = async (source: string): Promise<AstResult> =>
    await parse(source, { file: "index.js" });

describe("comment validation", (): void => {
    it("derives value from source and does not emit range or loc", async (): Promise<void> => {
        const source: string = "// real\nconst a = 1;";
        const parsed: AstResult = await parsedSource(source);
        const parsedComment: DeepReadonly<Comment> | undefined =
            parsed.ast.program.comments?.[0];

        expect(parsedComment?.value).toBe(" real");
        expect(parsedComment).not.toHaveProperty("range");
        expect(parsedComment).not.toHaveProperty("loc");

        const program: Program = copyProgram(parsed.ast.program);
        program.comments = [
            {
                type: "Line",
                value: "bogus",
                start: 0,
                end: 7,
                range: [0, 7],
                loc: {
                    start: { line: 1, column: 0 },
                    end: { line: 1, column: 7 },
                },
            },
        ] as unknown as Array<Comment>;
        const rewritten: AstResult = await rewrite(program, source, {
            file: "index.js",
        });
        const rewrittenComment: DeepReadonly<Comment> | undefined =
            rewritten.ast.program.comments?.[0];
        const generated: CodeResult = await generate(rewritten.ast, {
            file: "index.js",
        });

        expect(rewrittenComment?.value).toBe(" real");
        expect(rewrittenComment).not.toHaveProperty("range");
        expect(rewrittenComment).not.toHaveProperty("loc");
        expect(generated.code).toContain("// real");
        expect(generated.code).not.toContain("bogus");
    });

    it("rejects comments without a complete span", async (): Promise<void> => {
        const source: string = "// a\nconst x = 1;";
        const parsed: AstResult = await parsedSource(source);
        const missing: Program = copyProgram(parsed.ast.program);
        missing.comments = [{ type: "Line" } as unknown as Comment];
        const missingResult: Promise<CodeResult> = generate(missing, source, {
            file: "index.js",
        });

        await expect(missingResult).rejects.toThrow(/comment 0/);
        await expect(missingResult).rejects.toThrow(/missing start\/end/);

        const loneStart: Program = copyProgram(parsed.ast.program);
        loneStart.comments = [{ type: "Line", start: 0 } as unknown as Comment];
        const loneStartResult: Promise<CodeResult> = generate(
            loneStart,
            source,
            { file: "index.js" },
        );

        await expect(loneStartResult).rejects.toThrow(/comment 0 \(start 0\)/);
        await expect(loneStartResult).rejects.toThrow(/missing start\/end/);
    });

    it("rejects a malformed block span with its offsets", async (): Promise<void> => {
        const source: string = "/* ok */\nconst x = 1;";
        const parsed: AstResult = await parsedSource(source);
        const program: Program = copyProgram(parsed.ast.program);
        program.comments = [
            { type: "Block", start: 0, end: 3 } as unknown as Comment,
        ];
        const result: Promise<CodeResult> = generate(program, source, {
            file: "index.js",
        });

        await expect(result).rejects.toThrow(/comment 0 \(start 0, end 3\)/);
        await expect(result).rejects.toThrow(/closing block delimiter/);
    });
});
