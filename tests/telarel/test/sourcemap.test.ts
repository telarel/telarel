import type { CompileResult, PluginContext } from "telarel";

import { compile } from "telarel";
import { describe, expect, it } from "vitest";

type ProgramFixture = {
    body: Array<{
        declarations: Array<{
            id: { type: string; name: string };
        }>;
    }>;
};

describe("sourcemap", (): void => {
    it("emits a v3 map with observed exact fields", async (): Promise<void> => {
        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [],
        });

        expect(typeof result.map).toBe("object");
        expect(result.map.version).toBe(3);
        // No plugin declares `transform`, so the compile takes the skip
        // path: verbatim code with a per-line identity map that names the
        // input file.
        expect(result.map.file).toBe("index.ts");
        expect(result.map.sourceRoot).toBe(void 0);
        expect(result.map.x_google_ignoreList).toBe(void 0);
        expect(result.map.sources).toEqual(["index.ts"]);
        expect(result.map.sourcesContent).toEqual(["const a = 1;"]);
        expect(result.map.names).toEqual([]);
        expect(result.map.mappings.length).toBeGreaterThan(0);
    });

    it("keeps the map stable after a transform mutation", async (): Promise<void> => {
        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                {
                    name: "renamer",
                    transform: (_ctx: PluginContext, args) => {
                        const program: ProgramFixture =
                            args.ast as unknown as ProgramFixture;
                        const statement:
                            | ProgramFixture["body"][number]
                            | undefined = program.body[0];
                        if (!statement) {
                            throw new Error("expected a top-level statement");
                        }
                        const declaration:
                            | ProgramFixture["body"][number]["declarations"][number]
                            | undefined = statement.declarations[0];
                        if (declaration?.id.type !== "Identifier") {
                            throw new Error(
                                "expected an identifier declaration",
                            );
                        }
                        declaration.id.name = "renamed";
                    },
                },
            ],
        });

        expect(result.code).toBe("const renamed = 1;");
        expect(result.map.version).toBe(3);
        expect(result.map.file).toBe(void 0);
        expect(result.map.sources).toEqual(["index.ts"]);
        expect(result.map.mappings.length).toBeGreaterThan(0);
    });

    it("compiles empty code to empty code and a minimal identity map", async (): Promise<void> => {
        // Skip path: the identity map emits one per-line token even for
        // empty source (`max(1)` line count).
        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "",
        });

        expect(result.code).toBe("");
        expect(result.map.mappings).toBe("AAAA");
    });

    it("round-trips BOM and non-ASCII code", async (): Promise<void> => {
        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: '\uFEFFconst µ = "日本語";',
        });

        // Skip path: output is verbatim, so the leading BOM survives in the
        // code wherever the runtime preserved it in the input (the WASI
        // TextDecoder strips a leading U+FEFF while the native binding
        // preserves it) — pin only the runtime-independent remainder.
        expect(result.code.replace(/^\uFEFF/gu, "")).toBe(
            'const µ = "日本語";',
        );
        expect(result.map.version).toBe(3);
        expect(result.map.sources).toEqual(["index.ts"]);
        // Skip path: identity mappings — one column-0 token per line.
        expect(result.map.mappings).toBe("AAAA");
        // `sourcesContent` mirrors the original text, but the leading BOM's
        // survival is runtime-dependent (the WASI TextDecoder strips a
        // leading U+FEFF while the native binding preserves it) — pin only
        // the runtime-independent remainder.
        expect(result.map.sourcesContent).not.toBe(void 0);
        expect(result.map.sourcesContent?.[0]?.replace(/^\uFEFF/gu, "")).toBe(
            'const µ = "日本語";',
        );
    });

    it("pins mappings across multiple lines", async (): Promise<void> => {
        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;\nconst b = 2;",
        });

        expect(result.code).toBe("const a = 1;\nconst b = 2;");
        expect(result.map.version).toBe(3);
        // Skip path: identity mappings — one column-0 token per line.
        expect(result.map.mappings).toBe("AAAA;AACA");
    });

    it("keeps CRLF in code and sourcesContent on the skip path", async (): Promise<void> => {
        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;\r\nconst b = 2;",
        });

        // Skip path: output code is verbatim, so CRLF line endings are
        // preserved instead of being normalized by codegen.
        expect(result.code).toBe("const a = 1;\r\nconst b = 2;");
        expect(result.map.version).toBe(3);
        expect(result.map.sourcesContent).toEqual([
            "const a = 1;\r\nconst b = 2;",
        ]);
        // Skip path: identity mappings — one column-0 token per line.
        expect(result.map.mappings).toBe("AAAA;AACA");
    });

    it("strips a trailing CRLF from code on the skip path", async (): Promise<void> => {
        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;\r\n",
        });

        // Skip path: the trailing `\r\n` terminator is stripped, but the
        // identity map still names the file and covers the line.
        expect(result.code).toBe("const a = 1;");
        expect(result.map.version).toBe(3);
        expect(result.map.mappings).toBe("AAAA");
    });

    it("uses the given file name as the single source", async (): Promise<void> => {
        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "entry.tsx",
            code: "const a = 1;",
        });

        expect(result.map.sources).toEqual(["entry.tsx"]);
        // Skip path: identity mappings — one column-0 token per line.
        expect(result.map.mappings).toBe("AAAA");
    });

    it("keeps mappings identical after a pass-through transform", async (): Promise<void> => {
        // A declared `transform` hook forces the codegen path (the skip
        // path only applies when no plugin uses `transform`), so the
        // baseline must also carry a transform hook. Both hooks mutate
        // nothing; their mappings must match exactly.
        const baseline: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                {
                    name: "baseline-pass-through",
                    transform: (): void => void 0,
                },
            ],
        });

        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                {
                    name: "pass-through",
                    transform: (): void => void 0,
                },
            ],
        });

        expect(result.map.mappings).toBe(baseline.map.mappings);
        expect(result.map.sources).toEqual(baseline.map.sources);
    });

    it("shifts mappings after a length-changing transform", async (): Promise<void> => {
        const baseline: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [],
        });

        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                {
                    name: "renamer",
                    transform: (_ctx: PluginContext, args) => {
                        const program: ProgramFixture =
                            args.ast as unknown as ProgramFixture;

                        const statement:
                            | ProgramFixture["body"][number]
                            | undefined = program.body[0];

                        if (!statement) {
                            throw new Error("expected a top-level statement");
                        }

                        const declaration:
                            | ProgramFixture["body"][number]["declarations"][number]
                            | undefined = statement.declarations[0];

                        if (declaration?.id.type !== "Identifier") {
                            throw new Error(
                                "expected an identifier declaration",
                            );
                        }

                        declaration.id.name = "renamed-long";
                    },
                },
            ],
        });

        expect(result.code).toBe("const renamed-long = 1;");
        expect(result.map.version).toBe(3);
        expect(result.map.sources).toEqual(["index.ts"]);
        expect(result.map.names.length).toBeGreaterThan(0);
        expect(result.map.mappings.length).toBeGreaterThan(0);
        // A changed AST must not produce byte-identical mappings: the
        // generated code is longer, so the later segments' column deltas
        // shift. The no-plugins baseline takes the skip path ("AAAA"),
        // while the mutated AST goes through codegen and produces
        // multi-segment mappings.
        expect(result.map.mappings).not.toBe(baseline.map.mappings);
    });

    it("pins exact mappings across three lines", async (): Promise<void> => {
        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;\nconst b = 2;\nconst c = 3;",
        });

        expect(result.code).toBe("const a = 1;\nconst b = 2;\nconst c = 3;");
        expect(result.map.version).toBe(3);
        // Skip path: identity mappings — one column-0 token per line.
        expect(result.map.mappings).toBe("AAAA;AACA;AACA");
    });
});
