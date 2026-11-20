import type {
    CodeResult,
    PluginContext,
    Program,
    TransformArgs,
    TransformResult,
} from "telarel";

import type { ProgramFixture } from "#/functions/ast";

import { compile } from "telarel";
import { describe, expect, it } from "vitest";

import { renameRootIdentifier } from "#/functions/ast";

describe("source map codegen", (): void => {
    it("emits a v3 codegen map with observed exact fields", async (): Promise<void> => {
        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [],
        });

        expect(typeof result.map).toBe("object");
        expect(result.map?.version).toBe(3);
        // Parse always runs, so the map is the real codegen map. It names the
        // input file as its single source, records the resolved output file,
        // and carries the encoded mappings.
        expect(result.map?.file).toBe("index.ts");
        expect(result.map?.sourceRoot).toBe(void 0);
        expect(result.map?.ignoreList).toBe(void 0);
        expect(result.map?.sources).toEqual(["index.ts"]);
        expect(result.map?.sourcesContent).toEqual(["const a = 1;"]);
        expect(result.map?.names).toEqual([]);
        expect(result.map?.mappings.length).toBeGreaterThan(0);
    });

    it("compiles empty code to empty code and an empty codegen map", async (): Promise<void> => {
        const result: CodeResult = await compile("", {
            cwd: "/repo",
            file: "index.ts",
        });

        expect(result.code).toBe("");
        expect(result.map?.mappings).toBe("");
    });

    it("round-trips BOM and non-ASCII code", async (): Promise<void> => {
        const result: CodeResult = await compile('\uFEFFconst µ = "日本語";', {
            cwd: "/repo",
            file: "index.ts",
        });

        // Codegen prints the program, so it drops the leading BOM from the
        // output. The WASI code path also strips a leading U+FEFF.
        expect(result.code.replace(/^\uFEFF/gu, "")).toBe(
            'const µ = "日本語";',
        );
        expect(result.map?.version).toBe(3);
        expect(result.map?.sources).toEqual(["index.ts"]);
        expect(result.map?.mappings).toBe("AAAC,MAAM,IAAI");
        // `sourcesContent` mirrors the original text, but whether the leading
        // BOM survives depends on the runtime. The WASI TextDecoder strips a
        // leading U+FEFF, and the native binding preserves it. Pin only the
        // runtime-independent remainder.
        expect(result.map?.sourcesContent).not.toBe(void 0);
        expect(result.map?.sourcesContent?.[0]?.replace(/^\uFEFF/gu, "")).toBe(
            'const µ = "日本語";',
        );
    });

    it("emits mappings for non-ASCII code through the transform path", async (): Promise<void> => {
        const result: CodeResult = await compile('const µ = "日本語";', {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "pass-through",
                    transform: (): void => void 0,
                },
            ],
        });

        expect(result.map?.version).toBe(3);
        expect(result.map?.mappings.length).toBeGreaterThan(0);
    });

    it("pins mappings across multiple lines", async (): Promise<void> => {
        const result: CodeResult = await compile("const a = 1;\nconst b = 2;", {
            cwd: "/repo",
            file: "index.ts",
        });

        expect(result.code).toBe("const a = 1;\nconst b = 2;");
        expect(result.map?.version).toBe(3);
        // Codegen emits one statement per generated line. The three segments on
        // each line mark the start of the statement, the identifier, and the
        // literal.
        expect(result.map?.mappings).toBe("AAAA,MAAM,IAAI;AACV,MAAM,IAAI");
    });

    it("normalizes CRLF in code but keeps it in sourcesContent", async (): Promise<void> => {
        const result: CodeResult = await compile(
            "const a = 1;\r\nconst b = 2;",
            {
                cwd: "/repo",
                file: "index.ts",
            },
        );

        // Codegen prints the program, so it normalizes CRLF line endings to LF
        // in the output. `sourcesContent` keeps the original text.
        expect(result.code).toBe("const a = 1;\nconst b = 2;");
        expect(result.map?.version).toBe(3);
        expect(result.map?.sourcesContent).toEqual([
            "const a = 1;\r\nconst b = 2;",
        ]);
        expect(result.map?.mappings).toBe("AAAA,MAAM,IAAI;AACV,MAAM,IAAI");
    });

    it("strips a trailing CRLF from code and emits codegen mappings", async (): Promise<void> => {
        const result: CodeResult = await compile("const a = 1;\r\n", {
            cwd: "/repo",
            file: "index.ts",
        });

        // Codegen strips the trailing `\r\n` terminator, and the map still
        // covers the single statement.
        expect(result.code).toBe("const a = 1;");
        expect(result.map?.version).toBe(3);
        expect(result.map?.mappings).toBe("AAAA,MAAM,IAAI");
    });

    it("uses the given file name as the single source", async (): Promise<void> => {
        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "entry.tsx",
        });

        expect(result.map?.sources).toEqual(["entry.tsx"]);
        expect(result.map?.mappings).toBe("AAAA,MAAM,IAAI");
    });

    it("keeps mappings identical after a pass-through transform", async (): Promise<void> => {
        const baseline: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "baseline-pass-through",
                    transform: (): void => void 0,
                },
            ],
        });

        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "pass-through",
                    transform: (): void => void 0,
                },
            ],
        });

        expect(result.map?.mappings).toBe(baseline.map?.mappings);
        expect(result.map?.sources).toEqual(baseline.map?.sources);
    });

    it("shifts mappings after a length-changing transform", async (): Promise<void> => {
        const baseline: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [],
        });

        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "renamer",
                    transform: (
                        _ctx: PluginContext,
                        args: TransformArgs,
                    ): TransformResult => {
                        args.ast.mutate((program: Program): void => {
                            renameRootIdentifier({
                                program: program as unknown as ProgramFixture,
                                name: "renamed-long",
                            });
                        });

                        return { ast: args.ast };
                    },
                },
            ],
        });

        expect(result.code).toBe("const renamed-long = 1;");
        expect(result.map?.version).toBe(3);
        expect(result.map?.sources).toEqual(["index.ts"]);
        // Codegen records the source identifier name that the renamed output
        // maps back to. The source text still reads `a`.
        expect(result.map?.names).toEqual(["a"]);
        expect(result.map?.mappings.length).toBeGreaterThan(0);
        // A changed AST must not produce byte-identical mappings. The generated
        // identifier is longer, so the column deltas of later segments shift.
        expect(result.map?.mappings).not.toBe(baseline.map?.mappings);
    });

    it("pins exact mappings across three lines", async (): Promise<void> => {
        const result: CodeResult = await compile(
            "const a = 1;\nconst b = 2;\nconst c = 3;",
            {
                cwd: "/repo",
                file: "index.ts",
            },
        );

        expect(result.code).toBe("const a = 1;\nconst b = 2;\nconst c = 3;");
        expect(result.map?.version).toBe(3);
        expect(result.map?.mappings).toBe(
            "AAAA,MAAM,IAAI;AACV,MAAM,IAAI;AACV,MAAM,IAAI",
        );
    });
});
