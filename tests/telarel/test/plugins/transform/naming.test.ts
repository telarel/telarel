import type { CodeResult } from "telarel";

import { compile } from "telarel";
import { transform } from "telarel/plugins/transform";
import { describe, expect, it } from "vitest";

import { SOURCE_MAPPING_COMMENT } from "#/functions/sourcemap";

describe("transform output file naming", (): void => {
    it("names the .ts output and external map URL after the transform", async (): Promise<void> => {
        const result: CodeResult = await compile("const a: number = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [transform()],
            sourceMap: { output: "external" },
        });

        expect(result.code).toBe(
            `const a = 1;\n${SOURCE_MAPPING_COMMENT}index.js.map\n`,
        );
        expect(result.map?.file).toBe("index.js");
        expect(result.map?.sources).toEqual(["index.ts"]);
    });

    it("keeps .ts output when no transform plugin runs", async (): Promise<void> => {
        const result: CodeResult = await compile("const a: number = 1;", {
            cwd: "/repo",
            file: "index.ts",
            sourceMap: { output: "external" },
        });

        expect(result.code).toBe(
            `const a: number = 1;\n${SOURCE_MAPPING_COMMENT}index.ts.map\n`,
        );
        expect(result.map?.file).toBe("index.ts");
        expect(result.map?.sources).toEqual(["index.ts"]);
    });

    it("preserves .tsx JSX, strips TypeScript, and names the output .jsx", async (): Promise<void> => {
        const result: CodeResult = await compile(
            "const node: JSX.Element = <Button>hi</Button>;",
            {
                cwd: "/repo",
                file: "index.tsx",
                plugins: [transform()],
                sourceMap: { output: "external" },
            },
        );

        expect(result.code).toBe(
            `const node = <Button>hi</Button>;\n${SOURCE_MAPPING_COMMENT}index.jsx.map\n`,
        );
        expect(result.map?.file).toBe("index.jsx");
        expect(result.map?.sources).toEqual(["index.tsx"]);
    });

    it("lowers .tsx JSX with the automatic runtime and names the output .js", async (): Promise<void> => {
        const result: CodeResult = await compile(
            "const node: JSX.Element = <Button>hi</Button>;",
            {
                cwd: "/repo",
                file: "index.tsx",
                plugins: [transform({ jsx: { runtime: "automatic" } })],
            },
        );

        expect(result.code).not.toContain("<Button>");
        expect(result.map?.file).toBe("index.js");
        expect(result.map?.sources).toEqual(["index.tsx"]);
    });

    it("lowers .jsx JSX with the automatic runtime and names the output .js", async (): Promise<void> => {
        const result: CodeResult = await compile(
            "const node = <Button>hi</Button>;",
            {
                cwd: "/repo",
                file: "index.jsx",
                plugins: [transform({ jsx: { runtime: "automatic" } })],
                sourceMap: { output: "external" },
            },
        );

        expect(result.code).not.toContain("<Button>");
        expect(result.code).toContain(
            `${SOURCE_MAPPING_COMMENT}index.js.map\n`,
        );
        expect(result.map?.file).toBe("index.js");
        expect(result.map?.sources).toEqual(["index.jsx"]);
    });

    it("lets an explicit output.file override the derived name", async (): Promise<void> => {
        const result: CodeResult = await compile("const a: number = 1;", {
            cwd: "/repo",
            file: "index.ts",
            output: { file: "dist/custom.mjs" },
            plugins: [transform()],
            sourceMap: { output: "external" },
        });

        expect(result.code).toBe(
            `const a = 1;\n${SOURCE_MAPPING_COMMENT}dist/custom.mjs.map\n`,
        );
        expect(result.map?.file).toBe("dist/custom.mjs");
        expect(result.map?.sources).toEqual(["index.ts"]);
    });
});
