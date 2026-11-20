import type { Ast, CodeResult, Generator } from "telarel";

import { createGenerator, parse } from "telarel";
import { describe, expect, it } from "vitest";

import {
    resolvedGeneratorOptions,
    resolvedOxcCodegenOptions,
} from "#/functions/options";

describe("createGenerator", (): void => {
    it("createGenerator prefers the AST language over the file", async (): Promise<void> => {
        const ast: Ast = (
            await parse("const f = <T,>(x: T) => x;", {
                language: "tsx",
            })
        ).ast;
        const generator: Generator = createGenerator(
            ast,
            resolvedGeneratorOptions(),
        );
        const result: CodeResult = await generator.generate();

        expect(result.code).toBe("const f = <T,>(x: T) => x;");
    });

    it("lets createGenerator apply curated options over raw oxc", async (): Promise<void> => {
        const ast: Ast = (
            await parse('function f() {\n    return "hello";\n}', {
                file: "index.ts",
            })
        ).ast;
        const generator: Generator = createGenerator(
            ast,
            resolvedGeneratorOptions({
                singleQuote: true,
                oxc: resolvedOxcCodegenOptions({
                    singleQuote: false,
                    indentChar: "tab",
                }),
            }),
        );
        const result: CodeResult = await generator.generate();

        expect(result.code).toBe("function f() {\n  return 'hello';\n}");
    });
});
