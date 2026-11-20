import type { PluginContext, TransformArgs } from "telarel";

import { CompileError, ErrorSource, PluginError, compile } from "telarel";
import { describe, expect, it } from "vitest";

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

describe("PluginError", (): void => {
    it("preserves a plugin error source through the JS-to-Rust marker", async (): Promise<void> => {
        const sourceCode: string = "const x = 1;";
        const error: CompileError = asCompileError(
            await rejection(
                compile("const a = 1;", {
                    cwd: "/repo",
                    file: "index.ts",
                    plugins: [
                        {
                            name: "boom-with-source",
                            transform: (): never => {
                                throw new PluginError("plugin-boom", {
                                    source: ErrorSource.fromCode(
                                        "input.ts",
                                        sourceCode,
                                        { start: 6, end: 7 },
                                    ),
                                });
                            },
                        },
                    ],
                }),
            ),
        );

        expect(error.name).toBe("CompileError");
        expect(error.message).toBe("plugin-boom");
        expect(error.render()).toContain("plugin-boom");
        expect(error.render()).toContain("input.ts");
        expect(error.source?.file).toBe("input.ts");
        expect(error.source?.span).toEqual({ start: 6, end: 7 });
        expect(error.source?.code).toBe(sourceCode);
    });

    it("preserves an AST-derived plugin error source", async (): Promise<void> => {
        const sourceCode: string = "const x = 1;";
        const error: CompileError = asCompileError(
            await rejection(
                compile(sourceCode, {
                    cwd: "/repo",
                    file: "index.ts",
                    plugins: [
                        {
                            name: "boom-from-ast",
                            transform: (
                                _ctx: PluginContext,
                                args: TransformArgs,
                            ): never => {
                                throw new PluginError("ast-boom", {
                                    source: ErrorSource.fromAst(
                                        "input.ts",
                                        args.ast,
                                        { start: 6, end: 7 },
                                    ),
                                });
                            },
                        },
                    ],
                }),
            ),
        );

        expect(error.message).toBe("ast-boom");
        expect(error.render()).toContain("input.ts");
        expect(error.source?.file).toBe("input.ts");
        expect(error.source?.span).toEqual({ start: 6, end: 7 });
        expect(error.source?.code).toBe(sourceCode);
    });

    it("leaves a source-less plugin error without file or span", async (): Promise<void> => {
        const error: CompileError = asCompileError(
            await rejection(
                compile("const a = 1;", {
                    cwd: "/repo",
                    file: "index.ts",
                    plugins: [
                        {
                            name: "boom-source-less",
                            transform: (): never => {
                                throw new PluginError("plain-boom");
                            },
                        },
                    ],
                }),
            ),
        );

        expect(error.message).toContain("plain-boom");
        expect(error.source).toBeUndefined();
    });

    it("converts a PluginError to a CompileError", (): void => {
        const source: ErrorSource = ErrorSource.fromCode(
            "input.ts",
            "const x = 1;",
            { start: 6, end: 7 },
        );
        const pluginError: PluginError = new PluginError("plugin-boom", {
            source,
        });
        const converted: CompileError = pluginError.toCompileError();

        expect(converted).toBeInstanceOf(CompileError);
        expect(converted.name).toBe("CompileError");
        expect(converted.message).toBe("plugin-boom");
        expect(converted.render()).toContain("input.ts");
        expect(converted.source?.file).toBe("input.ts");
        expect(converted.source?.span).toEqual({ start: 6, end: 7 });
        expect(converted.source?.code).toBe("const x = 1;");

        const bare: CompileError = new PluginError(
            "plain-boom",
        ).toCompileError();

        expect(bare.message).toBe("plain-boom");
        expect(bare.render()).toBe("plain-boom");
        expect(bare.source).toBeUndefined();
    });

    it("converts a PluginError through CompileError.from", (): void => {
        const source: ErrorSource = ErrorSource.fromCode(
            "input.ts",
            "const x = 1;",
            { start: 6, end: 7 },
        );
        const converted: CompileError = CompileError.from(
            new PluginError("plugin-boom", { source }),
        );

        expect(converted).toBeInstanceOf(CompileError);
        expect(converted.source?.file).toBe("input.ts");
        expect(converted.source?.span).toEqual({ start: 6, end: 7 });
        expect(converted.source?.code).toBe("const x = 1;");
    });
});
