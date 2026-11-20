import type {
    CodeResult,
    CompileOptions,
    Language,
    PluginContext,
    StartArgs,
} from "telarel";

import { CompileError, compile } from "telarel";
import { describe, expect, it } from "vitest";

describe("parse language", (): void => {
    it("compiles ts syntax on a js file with an explicit ts language", async (): Promise<void> => {
        // A `transform` hook forces the parse path. With the explicit
        // `language: "ts"`, the ts grammar applies despite the `.js` file, so
        // the annotated declaration parses and compiles to itself.
        const result: CodeResult = await compile("const a: number = 1;", {
            file: "index.js",
            language: "ts",
            plugins: [
                {
                    name: "force-parse",
                    transform: (): void => void 0,
                },
            ],
        });

        expect(result.code).toBe("const a: number = 1;");
        expect(result.map?.version).toBe(3);
        expect(result.map?.mappings.length).toBeGreaterThan(0);
    });

    it("rejects ts syntax on a ts file with an explicit js language", async (): Promise<void> => {
        // An explicit `language` wins over extension inference. The js grammar
        // cannot parse the annotation, so the parse fails despite the `.ts`
        // file.
        // The error keeps the bare parser message. `render()` adds the
        // diagnostic in code-frame style, with the file name and caret
        // position.
        await expect(
            compile("const a: number = 1;", {
                file: "index.ts",
                language: "js",
                plugins: [
                    {
                        name: "force-parse",
                        transform: (): void => void 0,
                    },
                ],
            }),
        ).rejects.toSatisfy(
            (error: unknown): boolean =>
                error instanceof CompileError &&
                error.render().includes("index.ts"),
        );

        await expect(
            compile("const a: number = 1;", {
                file: "index.ts",
                language: "js",
                plugins: [
                    {
                        name: "force-parse",
                        transform: (): void => void 0,
                    },
                ],
            }),
        ).rejects.toThrow("Missing initializer in const declaration");
    });

    it("infers the ts grammar from the ts extension when language is omitted", async (): Promise<void> => {
        const result: CodeResult = await compile("const a: number = 1;", {
            file: "index.ts",
            plugins: [
                {
                    name: "force-parse",
                    transform: (): void => void 0,
                },
            ],
        });

        expect(result.code).toBe("const a: number = 1;");
    });

    it("defaults extensionless files to plain js when language is omitted", async (): Promise<void> => {
        // Without an extension, the default js grammar applies as an es
        // module. Plain js parses, and ts-only syntax fails.
        const result: CodeResult = await compile("const a = 1;", {
            file: "index",
            plugins: [
                {
                    name: "force-parse",
                    transform: (): void => void 0,
                },
            ],
        });

        expect(result.code).toBe("const a = 1;");

        await expect(
            compile("const a: number = 1;", {
                file: "index",
                plugins: [
                    {
                        name: "force-parse",
                        transform: (): void => void 0,
                    },
                ],
            }),
        ).rejects.toSatisfy(
            (error: unknown): boolean =>
                error instanceof CompileError &&
                error.render().includes("index"),
        );

        await expect(
            compile("const a: number = 1;", {
                file: "index",
                plugins: [
                    {
                        name: "force-parse",
                        transform: (): void => void 0,
                    },
                ],
            }),
        ).rejects.toThrow("Missing initializer in const declaration");
    });

    it("parses declaration syntax with an explicit dts language", async (): Promise<void> => {
        // The dts grammar is the `.d.ts` declaration grammar, so ambient
        // declarations parse even on a `.js` file.
        const result: CodeResult = await compile("declare const a: number;", {
            file: "index.js",
            language: "dts",
            plugins: [
                {
                    name: "force-parse",
                    transform: (): void => void 0,
                },
            ],
        });

        expect(result.code).toBe("declare const a: number;");
        expect(result.map?.mappings.length).toBeGreaterThan(0);
    });

    it("round-trips every language label through the compile boundary", async (): Promise<void> => {
        const samples: Array<{ code: string; language: Language }> = [
            { code: "const a = 1;", language: "js" },
            { code: "const a: number = 1;", language: "ts" },
            { code: "declare const a: number;", language: "dts" },
            { code: "const a = <Button />;", language: "jsx" },
            { code: "const a: number = <Button />;", language: "tsx" },
        ];

        const seen: Array<Language> = [];

        for (const sample of samples) {
            const result: CodeResult = await compile(sample.code, {
                file: "index.js",
                language: sample.language,
                plugins: [
                    {
                        name: "observe",
                        start: (_ctx: PluginContext, args: StartArgs): void => {
                            seen.push(args.options.language);
                        },
                        transform: (): void => void 0,
                    },
                ],
            });

            expect(result.code).toBe(sample.code);
        }

        expect(seen).toEqual(["js", "ts", "dts", "jsx", "tsx"]);
    });

    it("rejects a language that is not js, ts, dts, jsx, or tsx", async (): Promise<void> => {
        // The napi bridge validates `language` before the pipeline runs. It
        // does so in the same style as the transform target parser.
        const json: unknown = {
            file: "index.js",
            language: "json",
        };

        const banana: unknown = {
            file: "index.js",
            language: "banana",
        };

        await expect(
            compile("const a = 1;", json as CompileOptions),
        ).rejects.toThrow(
            'invalid language "json": expected "js", "ts", "dts", "jsx" or "tsx"',
        );

        await expect(
            compile("const a = 1;", banana as CompileOptions),
        ).rejects.toThrow(
            'invalid language "banana": expected "js", "ts", "dts", "jsx" or "tsx"',
        );
    });

    it("compiles jsx syntax on a js file with an explicit jsx language", async (): Promise<void> => {
        const result: CodeResult = await compile(
            "const a = <Button>hi</Button>;",
            {
                file: "index.js",
                language: "jsx",
                plugins: [
                    {
                        name: "force-parse",
                        transform: (): void => void 0,
                    },
                ],
            },
        );

        expect(result.code).toBe("const a = <Button>hi</Button>;");
        expect(result.map?.mappings.length).toBeGreaterThan(0);
    });

    it("compiles tsx syntax on a js file with an explicit tsx language", async (): Promise<void> => {
        const result: CodeResult = await compile(
            "const a: number = <Button>hi</Button>;",
            {
                file: "index.js",
                language: "tsx",
                plugins: [
                    {
                        name: "force-parse",
                        transform: (): void => void 0,
                    },
                ],
            },
        );

        expect(result.code).toBe("const a: number = <Button>hi</Button>;");
        expect(result.map?.mappings.length).toBeGreaterThan(0);
    });
});
