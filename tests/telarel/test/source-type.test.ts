import type { CompileResult, Options } from "telarel";

import { compile } from "telarel";
import { describe, expect, it } from "vitest";

describe("language", (): void => {
    it("compiles ts syntax on a js file with an explicit ts language", async (): Promise<void> => {
        // A `transform` hook forces the parse path; with the explicit
        // `language: "ts"` the ts grammar applies despite the `.js`
        // file, so the annotated declaration parses and compiles to
        // itself.
        const result: CompileResult = await compile({
            file: "index.js",
            code: "const a: number = 1;",
            language: "ts",
            plugins: [
                {
                    name: "force-parse",
                    transform: (): void => void 0,
                },
            ],
        });

        expect(result.code).toBe("const a: number = 1;");
        expect(result.map.version).toBe(3);
        expect(result.map.mappings.length).toBeGreaterThan(0);
    });

    it("rejects ts syntax on a ts file with an explicit js language", async (): Promise<void> => {
        // Explicit `language` wins over extension inference: the js
        // grammar cannot parse the annotation, so the parse fails despite
        // the `.ts` file.
        // Pinned observed message: the native parser renders a code-frame
        // style diagnostic including the file name and caret position.
        await expect(
            compile({
                file: "index.ts",
                code: "const a: number = 1;",
                language: "js",
                plugins: [
                    {
                        name: "force-parse",
                        transform: (): void => void 0,
                    },
                ],
            }),
        ).rejects.toThrow("index.ts");

        await expect(
            compile({
                file: "index.ts",
                code: "const a: number = 1;",
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
        const result: CompileResult = await compile({
            file: "index.ts",
            code: "const a: number = 1;",
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
        // Pinned observed behavior: without an extension the default es
        // module js grammar applies, so plain js parses while ts-only
        // syntax is rejected.
        const result: CompileResult = await compile({
            file: "index",
            code: "const a = 1;",
            plugins: [
                {
                    name: "force-parse",
                    transform: (): void => void 0,
                },
            ],
        });

        expect(result.code).toBe("const a = 1;");

        await expect(
            compile({
                file: "index",
                code: "const a: number = 1;",
                plugins: [
                    {
                        name: "force-parse",
                        transform: (): void => void 0,
                    },
                ],
            }),
        ).rejects.toThrow("index");

        await expect(
            compile({
                file: "index",
                code: "const a: number = 1;",
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
        const result: CompileResult = await compile({
            file: "index.js",
            code: "declare const a: number;",
            language: "dts",
            plugins: [
                {
                    name: "force-parse",
                    transform: (): void => void 0,
                },
            ],
        });

        expect(result.code).toBe("declare const a: number;");
        expect(result.map.mappings.length).toBeGreaterThan(0);
    });

    it("rejects a language that is not js, ts, dts, jsx, or tsx", async (): Promise<void> => {
        // Pinned observed behavior: the napi bridge validates the language
        // before the pipeline runs, in the same style as the transform
        // target parser.
        const json: unknown = {
            file: "index.js",
            code: "const a = 1;",
            language: "json",
        };

        const banana: unknown = {
            file: "index.js",
            code: "const a = 1;",
            language: "banana",
        };

        await expect(compile(json as Options)).rejects.toThrow(
            'invalid language "json": expected "js", "ts", "dts", "jsx" or "tsx"',
        );

        await expect(compile(banana as Options)).rejects.toThrow(
            'invalid language "banana": expected "js", "ts", "dts", "jsx" or "tsx"',
        );
    });

    it("compiles jsx syntax on a js file with an explicit jsx language", async (): Promise<void> => {
        const result: CompileResult = await compile({
            file: "index.js",
            code: "const a = <Button>hi</Button>;",
            language: "jsx",
            plugins: [
                {
                    name: "force-parse",
                    transform: (): void => void 0,
                },
            ],
        });

        expect(result.code).toBe("const a = <Button>hi</Button>;");
        expect(result.map.mappings.length).toBeGreaterThan(0);
    });

    it("compiles tsx syntax on a js file with an explicit tsx language", async (): Promise<void> => {
        const result: CompileResult = await compile({
            file: "index.js",
            code: "const a: number = <Button>hi</Button>;",
            language: "tsx",
            plugins: [
                {
                    name: "force-parse",
                    transform: (): void => void 0,
                },
            ],
        });

        expect(result.code).toBe("const a: number = <Button>hi</Button>;");
        expect(result.map.mappings.length).toBeGreaterThan(0);
    });
});

describe("source type", (): void => {
    it("rejects top-level await with an explicit script source type", async (): Promise<void> => {
        // Pinned observed message: the native parser renders a code-frame
        // style diagnostic including the file name and caret position.
        await expect(
            compile({
                file: "index.js",
                code: "await Promise.resolve();",
                sourceType: "script",
                plugins: [
                    {
                        name: "force-parse",
                        transform: (): void => void 0,
                    },
                ],
            }),
        ).rejects.toThrow("index.js");

        await expect(
            compile({
                file: "index.js",
                code: "await Promise.resolve();",
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
        const result: CompileResult = await compile({
            file: "index.js",
            code: "await Promise.resolve();",
            sourceType: "module",
            plugins: [
                {
                    name: "force-parse",
                    transform: (): void => void 0,
                },
            ],
        });

        expect(result.code).toBe("await Promise.resolve();");
        expect(result.map.mappings.length).toBeGreaterThan(0);
    });

    it("auto-detects the module goal with an unambiguous source type", async (): Promise<void> => {
        // Pinned observed behavior: in unambiguous mode the parser upgrades
        // the file to the module goal when it sees unambiguously-module
        // syntax (here the top-level await), so the script-only error is
        // deferred away and the code compiles.
        const result: CompileResult = await compile({
            file: "index.js",
            code: "await Promise.resolve();",
            sourceType: "unambiguous",
            plugins: [
                {
                    name: "force-parse",
                    transform: (): void => void 0,
                },
            ],
        });

        expect(result.code).toBe("await Promise.resolve();");

        const withEsmSyntax: CompileResult = await compile({
            file: "index.js",
            code: "export const a = 1;",
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
        // A top-level `return` is only valid in commonjs files; the parse
        // failing pins that the unambiguous resolution of plain code is
        // script, not commonjs.
        await expect(
            compile({
                file: "index.js",
                code: "if (true) { return; }",
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
        // Pinned observed behavior for the `commonjs` module kind: oxc
        // parses the file as if wrapped in a function body, so a top-level
        // `return` (and `new.target`) is valid, while `import.meta` and
        // top-level await are not.
        const result: CompileResult = await compile({
            file: "index.js",
            code: "if (true) { return; }",
            sourceType: "commonjs",
            plugins: [
                {
                    name: "force-parse",
                    transform: (): void => void 0,
                },
            ],
        });

        expect(result.code).toBe("if (true) {\n\treturn;\n}");
        expect(result.map.mappings.length).toBeGreaterThan(0);
    });

    it("rejects import.meta with a commonjs source type", async (): Promise<void> => {
        // Pins that `commonjs` is not the module goal: `import.meta` is
        // rejected the same way as in script mode.
        await expect(
            compile({
                file: "index.js",
                code: "const url = import.meta.url;",
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
        // `sourceType` is orthogonal to the grammar: the ts grammar still
        // applies to the `.ts` file, but the module kind comes from
        // `sourceType`, so top-level await is rejected where the default
        // unambiguous resolution would have accepted it.
        await expect(
            compile({
                file: "index.ts",
                code: "await Promise.resolve();",
                sourceType: "script",
                plugins: [
                    {
                        name: "force-parse",
                        transform: (): void => void 0,
                    },
                ],
            }),
        ).rejects.toThrow("`await` is only allowed within async functions");

        const result: CompileResult = await compile({
            file: "index.ts",
            code: "await Promise.resolve();",
            plugins: [
                {
                    name: "force-parse",
                    transform: (): void => void 0,
                },
            ],
        });

        expect(result.code).toBe("await Promise.resolve();");
    });

    it("keeps module syntax parseable with an explicit script source type", async (): Promise<void> => {
        // Pinned observed behavior (oxc 0.150): the parser does not reject
        // `import` / `export` in script mode at parse time, so module
        // syntax survives even though the file is declared a script. The
        // script goal is enforced through module-goal-only constructs such
        // as top-level await and `import.meta` instead.
        const result: CompileResult = await compile({
            file: "index.js",
            code: "import a from 'mod';\nexport const b = a;",
            sourceType: "script",
            plugins: [
                {
                    name: "force-parse",
                    transform: (): void => void 0,
                },
            ],
        });

        expect(result.code).toBe('import a from "mod";\nexport const b = a;');
        expect(result.map.mappings.length).toBeGreaterThan(0);
    });

    it("rejects a sourceType that is not script, commonjs, module, or unambiguous", async (): Promise<void> => {
        // Pinned observed behavior: the napi bridge validates the source
        // type before the pipeline runs, in the same style as the
        // transform target parser. The old grammar values are no longer
        // valid source types.
        const js: unknown = {
            file: "index.js",
            code: "const a = 1;",
            sourceType: "js",
        };

        const banana: unknown = {
            file: "index.js",
            code: "const a = 1;",
            sourceType: "banana",
        };

        await expect(compile(js as Options)).rejects.toThrow(
            'invalid source type "js": expected "script", "commonjs", "module" or "unambiguous"',
        );

        await expect(compile(banana as Options)).rejects.toThrow(
            'invalid source type "banana": expected "script", "commonjs", "module" or "unambiguous"',
        );
    });
});
