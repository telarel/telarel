import type {
    AstOptions,
    AstResult,
    CodeResult,
    Language,
    Parser,
} from "telarel";

import { Ast, createParser, generate, parse, rewrite } from "telarel";
import { describe, expect, it } from "vitest";

import { resolvedParserOptions } from "#/functions/options";

type Program = AstOptions["program"];

describe("Ast language", (): void => {
    it("populates language from parse", async (): Promise<void> => {
        const explicit: AstResult = await parse("const a = 1;", {
            file: "index.js",
            language: "jsx",
        });
        const inferred: AstResult = await parse("const a: number = 1;", {
            file: "index.ts",
        });

        expect(explicit.ast.language).toBe("jsx");
        expect(inferred.ast.language).toBe("ts");
    });

    it("populates language from rewrite", async (): Promise<void> => {
        const source: string = "const a = 1;";
        const parsed: AstResult = await parse(source, { file: "index.ts" });
        const manual: Ast = new Ast({
            program: parsed.ast.program as unknown as Program,
            source,
        });

        expect(manual.language).toBeUndefined();

        const result: AstResult = await rewrite(manual, { file: "index.jsx" });

        expect(result.ast.language).toBe("jsx");
    });

    it("populates language from createParser", async (): Promise<void> => {
        const parser: Parser = createParser(
            "declare const a: number;",
            resolvedParserOptions({
                file: "index.js",
                language: "dts",
            }),
        );

        const result: AstResult = await parser.parse();

        expect(result.ast.language).toBe("dts");
    });

    it("round-trips every language label through parse, rewrite, and generate", async (): Promise<void> => {
        const samples: Array<{ language: Language; code: string }> = [
            { language: "js", code: "const a = 1;" },
            { language: "ts", code: "const a: number = 1;" },
            { language: "dts", code: "declare const a: number;" },
            { language: "jsx", code: "const a = <Button />;" },
            { language: "tsx", code: "const a: number = <Button />;" },
        ];

        for (const sample of samples) {
            const parsed: AstResult = await parse(sample.code, {
                file: "index.js",
                language: sample.language,
            });

            expect(parsed.ast.language).toBe(sample.language);

            const rewritten: AstResult = await rewrite(parsed.ast);

            expect(rewritten.ast.language).toBe(sample.language);

            const generated: CodeResult = await generate(rewritten.ast);

            expect(generated.code).toBe(sample.code);
        }
    });
});
