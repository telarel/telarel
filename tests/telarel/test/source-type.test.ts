import type {
    CodeResult,
    CompileOptions,
    Language,
    PluginContext,
    StartArgs,
} from "telarel";

import { CompileError, compile } from "telarel";
import { describe, expect, it } from "vitest";

describe("language", (): void => {
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

describe("source type", (): void => {
    it("rejects top-level await with an explicit script source type", async (): Promise<void> => {
        // The error keeps the bare parser message. `render()` adds the
        // diagnostic in code-frame style, with the file name and caret
        // position.
        await expect(
            compile("await Promise.resolve();", {
                file: "index.js",
                sourceType: "script",
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
                error.render().includes("index.js"),
        );

        await expect(
            compile("await Promise.resolve();", {
                file: "index.js",
                sourceType: "script",
                plugins: [
                    {
                        name: "force-parse",
                        transform: (): void => void 0,
                    },
                ],
            }),
        ).rejects.toThrow("`await` is only allowed within async functions");
    });

    it("allows top-level await with an explicit module source type", async (): Promise<void> => {
        const result: CodeResult = await compile("await Promise.resolve();", {
            file: "index.js",
            sourceType: "module",
            plugins: [
                {
                    name: "force-parse",
                    transform: (): void => void 0,
                },
            ],
        });

        expect(result.code).toBe("await Promise.resolve();");
        expect(result.map?.mappings.length).toBeGreaterThan(0);
    });

    it("auto-detects the module goal with an unambiguous source type", async (): Promise<void> => {
        // In unambiguous mode, the parser upgrades the file to the module goal
        // when it sees module-only syntax, here top-level await. The parser
        // then defers the script-only error, so the code compiles.
        const result: CodeResult = await compile("await Promise.resolve();", {
            file: "index.js",
            sourceType: "unambiguous",
            plugins: [
                {
                    name: "force-parse",
                    transform: (): void => void 0,
                },
            ],
        });

        expect(result.code).toBe("await Promise.resolve();");

        const withEsmSyntax: CodeResult = await compile("export const a = 1;", {
            file: "index.js",
            sourceType: "unambiguous",
            plugins: [
                {
                    name: "force-parse",
                    transform: (): void => void 0,
                },
            ],
        });

        expect(withEsmSyntax.code).toBe("export const a = 1;");
    });

    it("resolves an unambiguous source without module syntax to script", async (): Promise<void> => {
        // A top-level `return` is valid only in commonjs files. The failed
        // parse shows that plain code resolves to script in unambiguous mode,
        // not commonjs.
        await expect(
            compile("if (true) { return; }", {
                file: "index.js",
                sourceType: "unambiguous",
                plugins: [
                    {
                        name: "force-parse",
                        transform: (): void => void 0,
                    },
                ],
            }),
        ).rejects.toThrow(
            "A 'return' statement can only be used within a function body.",
        );
    });

    it("allows a top-level return with a commonjs source type", async (): Promise<void> => {
        // For the `commonjs` module kind, oxc parses the file as if a function
        // body wraps it. A top-level `return` and `new.target` are valid, while
        // `import.meta` and top-level await are not.
        const result: CodeResult = await compile("if (true) { return; }", {
            file: "index.js",
            sourceType: "commonjs",
            plugins: [
                {
                    name: "force-parse",
                    transform: (): void => void 0,
                },
            ],
        });

        expect(result.code).toBe("if (true) {\n\treturn;\n}");
        expect(result.map?.mappings.length).toBeGreaterThan(0);
    });

    it("rejects import.meta with a commonjs source type", async (): Promise<void> => {
        // Pins that `commonjs` is not the module goal: `import.meta` is
        // rejected the same way as in script mode.
        await expect(
            compile("const url = import.meta.url;", {
                file: "index.js",
                sourceType: "commonjs",
                plugins: [
                    {
                        name: "force-parse",
                        transform: (): void => void 0,
                    },
                ],
            }),
        ).rejects.toThrow("Unexpected import.meta expression");
    });

    it("overrides the grammar resolution with an explicit script source type on a ts file", async (): Promise<void> => {
        // `sourceType` is independent of the grammar. The ts grammar still
        // applies to the `.ts` file, but `sourceType` sets the module kind.
        // Top-level await fails here, even though the default unambiguous
        // resolution accepts it.
        await expect(
            compile("await Promise.resolve();", {
                file: "index.ts",
                sourceType: "script",
                plugins: [
                    {
                        name: "force-parse",
                        transform: (): void => void 0,
                    },
                ],
            }),
        ).rejects.toThrow("`await` is only allowed within async functions");

        const result: CodeResult = await compile("await Promise.resolve();", {
            file: "index.ts",
            plugins: [
                {
                    name: "force-parse",
                    transform: (): void => void 0,
                },
            ],
        });

        expect(result.code).toBe("await Promise.resolve();");
    });

    it("rejects module syntax with an explicit script source type", async (): Promise<void> => {
        // With oxc 0.153, the parser rejects `import` and `export` when the
        // file is a script. Module declarations can only appear at the top
        // level of a module.
        await expect(
            compile("import a from 'mod';\nexport const b = a;", {
                file: "index.js",
                sourceType: "script",
                plugins: [
                    {
                        name: "force-parse",
                        transform: (): void => void 0,
                    },
                ],
            }),
        ).rejects.toThrow("Cannot use import statement outside a module");
    });

    it("rejects a sourceType that is not script, commonjs, module, or unambiguous", async (): Promise<void> => {
        // The napi bridge validates the source type before the pipeline runs.
        // It does so in the same style as the transform target parser. The old
        // grammar values are no longer valid source types.
        const js: unknown = {
            file: "index.js",
            sourceType: "js",
        };

        const banana: unknown = {
            file: "index.js",
            sourceType: "banana",
        };

        await expect(
            compile("const a = 1;", js as CompileOptions),
        ).rejects.toThrow(
            'invalid source type "js": expected "script", "commonjs", "module" or "unambiguous"',
        );

        await expect(
            compile("const a = 1;", banana as CompileOptions),
        ).rejects.toThrow(
            'invalid source type "banana": expected "script", "commonjs", "module" or "unambiguous"',
        );
    });
});
