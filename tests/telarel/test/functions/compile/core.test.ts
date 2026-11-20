import type { CodeResult, PluginContext, TransformArgs } from "telarel";

import { compile } from "telarel";
import { describe, expect, it } from "vitest";

import { buildMap } from "#/functions/sourcemap";

type CallFixture = {
    body: Array<{
        expression: {
            callee: { object: { type: string } };
        };
    }>;
};

describe("compile", (): void => {
    it("returns original code with no plugins", async (): Promise<void> => {
        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
        });

        expect(result.code).toBe("const a = 1;");
        expect(result.map?.version).toBe(3);
        expect(result.map?.mappings.length).toBeGreaterThan(0);
    });

    it("applies generator options", async (): Promise<void> => {
        const result: CodeResult = await compile('const a = "hello";', {
            file: "index.ts",
            generator: {
                singleQuote: true,
                indentStyle: "space",
                indentWidth: 4,
            },
        });

        expect(result.code).toBe("const a = 'hello';");
    });

    it("defaults the file to index.ts with the ts grammar", async (): Promise<void> => {
        // The default file is `index.ts`, so the default grammar is ts: an
        // annotated declaration parses and round-trips without a transform.
        const result: CodeResult = await compile("const a: number = 1;");

        expect(result.code).toBe("const a: number = 1;");
        expect(result.map?.sources).toEqual(["index.ts"]);
    });

    it("honors explicit file and language overrides", async (): Promise<void> => {
        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "entry.jsx",
            language: "jsx",
        });

        expect(result.map?.sources).toEqual(["entry.jsx"]);
    });

    it("treats a pass-through transform hook as no change", async (): Promise<void> => {
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

        expect(result.code).toBe("const a = 1;");
        expect(result.map?.version).toBe(3);
        expect(result.map?.mappings.length).toBeGreaterThan(0);
    });

    it("runs a visitor from oxc-parser inside transform", async (): Promise<void> => {
        const seen: Array<string> = [];

        const result: CodeResult = await compile("console.log(1);", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "spy",
                    transform: (
                        _ctx: PluginContext,
                        args: TransformArgs,
                    ): void => {
                        const program: CallFixture = args.ast
                            .program as unknown as CallFixture;
                        const statement: CallFixture["body"][number] | void =
                            program.body[0];
                        if (!statement) {
                            throw new Error("expected a top-level statement");
                        }
                        seen.push(statement.expression.callee.object.type);
                    },
                },
            ],
        });

        expect(seen).toEqual(["Identifier"]);
        expect(result.code).toContain("console.log(1)");
        expect(result.map?.version).toBe(3);
        expect(result.map?.mappings.length).toBeGreaterThan(0);
    });

    it("composes a requested input map back to the upstream original", async (): Promise<void> => {
        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            sourceMap: {
                input: buildMap({
                    file: "upstream.ts",
                    mappings: "AASA",
                    names: [],
                    sources: ["upstream.ts"],
                    sourcesContent: ["const a = 1;"],
                }),
            },
        });

        // The input map resolves parse-input (0, 0) to original (9, 0).
        // The composed map takes its source identity from the input map.
        expect(result.map?.file).toBe("index.ts");
        expect(result.map?.sources).toEqual(["upstream.ts"]);
        expect(result.map?.sourcesContent).toEqual(["const a = 1;"]);
        expect(result.map?.mappings).toBe("AASA,MAAA,IAAA");
    });

    it("appends the external map URL derived from output.file", async (): Promise<void> => {
        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "src/index.ts",
            output: { file: "dist/index.js" },
            sourceMap: { output: "external" },
        });

        expect(result.code).toBe(
            "const a = 1;\n//# sourceMappingURL=dist/index.js.map\n",
        );
        expect(result.map?.sources).toEqual(["src/index.ts"]);
    });

    it("records output.file as map.file and keeps the input file as the source", async (): Promise<void> => {
        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "src/index.ts",
            output: { file: "dist/index.js" },
        });

        expect(result.map?.file).toBe("dist/index.js");
        expect(result.map?.sources).toEqual(["src/index.ts"]);
    });

    it("appends an inline map and drops the map end-to-end", async (): Promise<void> => {
        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            sourceMap: { output: "inline" },
        });

        expect(result.map).toBeNull();
        expect(result.code).toContain("//#" + " sourceMappingURL=");
    });
});
