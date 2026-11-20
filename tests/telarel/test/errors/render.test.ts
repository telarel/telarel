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

describe("render", (): void => {
    it("clamps out-of-range code-frame spans", (): void => {
        const error: CompileError = new CompileError("clamped-boom", {
            source: ErrorSource.fromCode("input.ts", "abc", {
                start: 99,
                end: 100,
            }),
        });

        const rendered: string = error.render();

        expect(error.message).toBe("clamped-boom");
        expect(rendered).toContain("clamped-boom");
        expect(rendered).toContain("input.ts");
        expect(rendered).not.toContain("NaN");
    });

    it("renders a code frame from a CompileError", (): void => {
        const source: ErrorSource = ErrorSource.fromCode(
            "input.ts",
            "const x = 1;",
            { start: 6, end: 7 },
        );
        const error: CompileError = new CompileError("rendered-boom", {
            source,
        });
        const rendered: string = error.render();

        expect(error.message).toBe("rendered-boom");
        expect(rendered).toContain("rendered-boom");
        expect(rendered).toContain("input.ts");
        expect(rendered).toContain("^");
        expect(rendered).toContain(",-[");
        expect(rendered).toContain("`----");
    });

    it("renders a multi-line span with the native renderer", (): void => {
        const code: string = "let a = 1;\nlet b = 2;";
        const error: CompileError = new CompileError("multi-line-boom", {
            source: ErrorSource.fromCode("input.ts", code, {
                start: 0,
                end: 14,
            }),
        });
        const rendered: string = error.render();

        expect(rendered).toContain("let a = 1;");
        expect(rendered).toContain("let b = 2;");
        expect(rendered).toContain(",->");
    });

    it("renders a code frame from a PluginError", (): void => {
        const source: ErrorSource = ErrorSource.fromCode(
            "input.ts",
            "const x = 1;",
            { start: 6, end: 7 },
        );
        const error: PluginError = new PluginError("rendered-boom", {
            source,
        });
        const rendered: string = error.render();

        expect(error.message).toBe("rendered-boom");
        expect(rendered).toContain("rendered-boom");
        expect(rendered).toContain("input.ts");
        expect(rendered).toContain("^");
        expect(rendered).toContain(",-[");
        expect(rendered).toContain("`----");
    });

    it("renders a PluginError inside a hook", async (): Promise<void> => {
        const sourceCode: string = "const x = 1;";
        let renderedInHook: string | undefined;

        const error: CompileError = asCompileError(
            await rejection(
                compile("const a = 1;", {
                    cwd: "/repo",
                    file: "index.ts",
                    plugins: [
                        {
                            name: "render-in-hook",
                            transform: (): never => {
                                const pluginError: PluginError =
                                    new PluginError("hook-boom", {
                                        source: ErrorSource.fromCode(
                                            "input.ts",
                                            sourceCode,
                                            { start: 6, end: 7 },
                                        ),
                                    });

                                renderedInHook = pluginError.render();

                                throw pluginError;
                            },
                        },
                    ],
                }),
            ),
        );

        expect(renderedInHook).toContain(",-[");
        expect(error.render()).toContain("hook-boom");
    });
});
