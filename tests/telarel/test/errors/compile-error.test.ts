import type { Ast, AstResult, Parser, Span } from "telarel";

import {
    CompileError,
    ErrorSource,
    compile,
    createParser,
    parse,
} from "telarel";
import { describe, expect, it } from "vitest";

import { resolvedParserOptions } from "#/functions/options";

const rejection = async (promise: Promise<unknown>): Promise<unknown> => {
    try {
        await promise;
    } catch (error: unknown) {
        return error;
    }

    throw new Error("expected the promise to reject");
};

const asCompileError = (error: unknown): CompileError => {
    expect(error).toBeInstanceOf(CompileError);

    return error as CompileError;
};

const compileError = (payload: Record<string, unknown>): string =>
    `__TELAREL_COMPILE_ERROR__:${JSON.stringify(payload)}`;

describe("CompileError", (): void => {
    it("rejects compile with a CompileError", async (): Promise<void> => {
        const error: CompileError = asCompileError(
            await rejection(
                compile("const = ;", { cwd: "/repo", file: "index.ts" }),
            ),
        );

        expect(error.name).toBe("CompileError");
        expect(error.render()).toContain("index.ts");
        expect(error.render()).toContain(",-[");
    });

    it("exposes a file and a UTF-16 span from a builtin parse error", async (): Promise<void> => {
        const code: string = "const 日本語 = 1; const = ;";
        const error: CompileError = asCompileError(
            await rejection(compile(code, { cwd: "/repo", file: "index.ts" })),
        );
        const source: ErrorSource | undefined = error.source;
        const span: Span | undefined = source?.span;

        expect(source?.file).toBe("index.ts");
        expect(source?.code).toBe(code);
        expect(span).toEqual({
            start: code.lastIndexOf("="),
            end: code.lastIndexOf("=") + 1,
        });
        expect(code.slice(span?.start, span?.end)).toBe("=");
    });

    it("converts emoji spans from UTF-8 bytes to UTF-16 units", async (): Promise<void> => {
        const code: string = "const 😀 = ;";
        const error: CompileError = asCompileError(
            await rejection(compile(code, { cwd: "/repo", file: "index.ts" })),
        );
        const start: number = code.indexOf("😀");
        const span: Span | undefined = error.source?.span;

        expect(span).toEqual({ start, end: start + "😀".length });
        expect(code.slice(span?.start, span?.end)).toBe("😀");
    });

    it("exposes the same span from createParser", async (): Promise<void> => {
        const code: string = "const 😀 = ;";
        const parser: Parser = createParser(
            code,
            resolvedParserOptions({
                language: "js",
                sourceType: "module",
            }),
        );
        const error: CompileError = asCompileError(
            await rejection(parser.parse()),
        );
        const start: number = code.indexOf("😀");

        expect(error.name).toBe("CompileError");
        expect(error.source?.file).toBe("index.ts");
        expect(error.source?.span).toEqual({
            start,
            end: start + "😀".length,
        });
    });

    it("returns an existing CompileError unchanged", (): void => {
        const error: CompileError = new CompileError("boom");

        expect(CompileError.from(error)).toBe(error);
    });

    it("rebuilds a raw binding compile error", (): void => {
        const raw: Error = new Error("raw-boom");

        Object.assign(raw, {
            name: "CompileError",
            source: {
                file: "input.ts",
                code: "const x = 1;",
                span: { start: 6, end: 7 },
            },
        });

        const error: CompileError = CompileError.from(raw);

        expect(error).toBeInstanceOf(CompileError);
        expect(error.name).toBe("CompileError");
        expect(error.message).toBe("raw-boom");
        expect(error.render()).toContain("input.ts");
        expect(error.source?.file).toBe("input.ts");
        expect(error.source?.span).toEqual({ start: 6, end: 7 });
        expect(error.source?.code).toBe("const x = 1;");
    });

    it("rebuilds a source-less raw binding compile error", (): void => {
        const raw: Error = new Error("raw-boom");

        Object.assign(raw, { name: "CompileError" });

        const error: CompileError = CompileError.from(raw);

        expect(error).toBeInstanceOf(CompileError);
        expect(error.message).toBe("raw-boom");
        expect(error.render()).toBe("raw-boom");
        expect(error.source).toBeUndefined();
    });

    it("decodes a raw binding error envelope as a fallback", (): void => {
        const raw: Error = new Error(
            compileError({
                name: "CompileError",
                message: "envelope-boom",
                source: {
                    file: "input.ts",
                    code: "const x = 1;",
                    span: { start: 6, end: 7 },
                },
            }),
        );
        const error: CompileError = CompileError.from(raw);

        expect(error).toBeInstanceOf(CompileError);
        expect(error.message).toBe("envelope-boom");
        expect(error.render()).toContain("input.ts");
        expect(error.message).not.toContain("__TELAREL_COMPILE_ERROR__");
        expect(error.source?.file).toBe("input.ts");
        expect(error.source?.span).toEqual({ start: 6, end: 7 });
        expect(error.source?.code).toBe("const x = 1;");
    });

    it("wraps messages from unknown errors", (): void => {
        expect(CompileError.from(new Error("plain-boom")).message).toBe(
            "plain-boom",
        );
        expect(CompileError.from("string-boom").message).toBe("string-boom");
    });

    it("creates immutable sources from code and AST", async (): Promise<void> => {
        const span: Span = { start: 1, end: 2 };
        const source: ErrorSource = ErrorSource.fromCode(
            "input.ts",
            "abc",
            span,
        );

        span.start = 99;

        expect(source.file).toBe("input.ts");
        expect(source.code).toBe("abc");
        expect(source.span).toEqual({ start: 1, end: 2 });

        const copy: Span = source.span;

        copy.end = 99;

        expect(source.span).toEqual({ start: 1, end: 2 });

        const parsed: AstResult = await parse("const x = 1;", {
            cwd: "/repo",
            file: "input.ts",
        });
        const ast: Ast = parsed.ast;
        const astSource: ErrorSource = ErrorSource.fromAst("input.ts", ast, {
            start: 6,
            end: 7,
        });

        expect(astSource.code).toBe(ast.source);
        expect(astSource.code).toBe("const x = 1;");
    });
});
