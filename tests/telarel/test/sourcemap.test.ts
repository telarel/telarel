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
        expect(result.map.file).toBe(void 0);
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
                        return { ast: args.ast };
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

    it("compiles empty code to empty code and empty mappings", async (): Promise<void> => {
        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "",
        });

        expect(result.code).toBe("");
        expect(result.map.mappings).toBe("");
    });

    it("round-trips BOM and non-ASCII code", async (): Promise<void> => {
        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: '\uFEFFconst µ = "日本語";',
        });

        expect(result.code).toBe('const µ = "日本語";');
        // Observed divergence between runtimes: the WASI runtime decodes
        // Rust strings with a TextDecoder whose default `ignoreBOM: false`
        // strips a LEADING U+FEFF, while the native binding preserves it.
        // The pipeline strips the BOM from output code on both runtimes, so
        // pin only the runtime-independent fields here.
        expect(result.map.version).toBe(3);
        expect(result.map.sources).toEqual(["index.ts"]);
        expect(result.map.mappings).toBe("AAAC,MAAM,IAAI");
        // `sourcesContent` mirrors the original text, but the leading BOM's
        // survival is runtime-dependent (the WASI TextDecoder strips a
        // leading U+FEFF while the native binding preserves it) — pin only
        // the runtime-independent remainder.
        expect(result.map.sourcesContent).not.toBe(void 0);
        if (result.map.sourcesContent) {
            const content: string | null | undefined =
                result.map.sourcesContent[0];
            expect(content?.replace(/^\uFEFF/gu, "")).toBe(
                'const µ = "日本語";',
            );
        }
    });

    it("pins mappings across multiple lines", async (): Promise<void> => {
        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;\nconst b = 2;",
        });

        expect(result.code).toBe("const a = 1;\nconst b = 2;");
        expect(result.map.version).toBe(3);
        expect(result.map.mappings).toBe("AAAA,MAAM,IAAI;AACV,MAAM,IAAI");
    });

    it("normalizes CRLF to LF in code while keeping CRLF in sourcesContent", async (): Promise<void> => {
        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;\r\nconst b = 2;",
        });

        expect(result.code).toBe("const a = 1;\nconst b = 2;");
        expect(result.map.version).toBe(3);
        expect(result.map.sourcesContent).toEqual([
            "const a = 1;\r\nconst b = 2;",
        ]);
        expect(result.map.mappings).toBe("AAAA,MAAM,IAAI;AACV,MAAM,IAAI");
    });

    it("uses the given file name as the single source", async (): Promise<void> => {
        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "entry.tsx",
            code: "const a = 1;",
        });

        expect(result.map.sources).toEqual(["entry.tsx"]);
        expect(result.map.mappings).toBe("AAAA,MAAM,IAAI");
    });

    it("keeps mappings identical after a pass-through transform", async (): Promise<void> => {
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
                        return { ast: args.ast };
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
        // shift. Observed: baseline "AAAA,MAAM,IAAI" vs "AAAA,MAAMA,eAAI".
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
        expect(result.map.mappings).toBe(
            "AAAA,MAAM,IAAI;AACV,MAAM,IAAI;AACV,MAAM,IAAI",
        );
    });
});
