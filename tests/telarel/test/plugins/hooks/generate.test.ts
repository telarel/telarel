import type {
    AstOptions,
    AstResult,
    CodeResult,
    GenerateArgs,
    Generator,
    ParseArgs,
    Parser,
    PluginContext,
    SourceMap,
    TransformArgs,
} from "telarel";

import {
    Ast,
    CompileError,
    compile,
    createGenerator,
    createParser,
} from "telarel";
import { describe, expect, it } from "vitest";

import {
    countSourceMappingComments,
    SOURCE_MAPPING_COMMENT,
} from "#/functions/sourcemap";

type CaptureProgramOptions = {
    code: string;
};

/**
 * Capture an AST with a pass-through compile for these hook tests.
 */
const captureProgram = async ({
    code,
}: CaptureProgramOptions): Promise<unknown> => {
    let snapshot: unknown;

    await compile(code, {
        cwd: "/repo",
        file: "index.ts",
        plugins: [
            {
                name: "snapshotter",
                transform: (_ctx: PluginContext, args: TransformArgs): void => {
                    snapshot = structuredClone(args.ast.program);
                },
            },
        ],
    });

    return snapshot;
};

type Program = AstOptions["program"];

describe("generate hooks", (): void => {
    it("generate: first non-null wins and later hooks are ignored", async (): Promise<void> => {
        const seen: Array<string> = [];

        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "gen-first",
                    generate: (): CodeResult => {
                        seen.push("first");

                        return { code: "first();", map: null };
                    },
                },
                {
                    name: "gen-second",
                    generate: (): CodeResult => {
                        seen.push("second");

                        return { code: "second();" };
                    },
                },
            ],
        });

        expect(seen).toEqual(["first"]);
        expect(result.code).toBe("first();");
        expect(result.map).toBeNull();
    });

    it("generate: a null return defers to the builtin generator", async (): Promise<void> => {
        const seen: Array<string> = [];

        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "gen-defer",
                    generate: (): null => {
                        seen.push("defer");

                        return null;
                    },
                },
            ],
        });

        expect(seen).toEqual(["defer"]);
        expect(result.code).toBe("const a = 1;");
        expect(result.map?.version).toBe(3);
    });

    it("generate: carries a returned map through", async (): Promise<void> => {
        const map: SourceMap = {
            version: 3,
            sources: ["index.ts"],
            sourcesContent: ["const a = 1;"],
            names: [],
            mappings: "AAAA",
        };

        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "gen-map",
                    generate: (): CodeResult => ({
                        code: "custom();",
                        map,
                    }),
                },
            ],
        });

        expect(result.code).toBe("custom();");
        expect(result.map?.mappings).toBe("AAAA");
        expect(result.map?.sources).toEqual(["index.ts"]);
    });

    it("generate: surfaces a generate hook error unchanged", async (): Promise<void> => {
        await expect(
            compile("const a = 1;", {
                cwd: "/repo",
                file: "index.ts",
                plugins: [
                    {
                        name: "boom-generate",
                        generate: (): never => {
                            throw new Error("generate-kaboom");
                        },
                    },
                ],
            }),
        ).rejects.toThrow("generate-kaboom");
    });

    it("drives a custom language through parse and generate hooks only", async (): Promise<void> => {
        // The custom source is invalid JS, so only plugin parse and generate
        // hooks can avoid both builtins.
        const custom: string = "greet world!";
        const program: Program = (await captureProgram({
            code: "const fromCustom = 1;",
        })) as Program;

        await expect(
            compile(custom, { cwd: "/repo", file: "index.ts" }),
        ).rejects.toSatisfy(
            (error: unknown): boolean =>
                error instanceof CompileError &&
                error.render().includes("index.ts"),
        );

        const result: CodeResult = await compile(custom, {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "custom-language",
                    parse: (): AstResult => ({
                        ast: new Ast({ program, source: custom }),
                    }),
                    generate: (
                        _ctx: PluginContext,
                        args: GenerateArgs,
                    ): CodeResult => {
                        // The generate hook sees the AST the parse hook put in
                        // place, not one built from `custom`.
                        expect(args.ast.source).toBe(custom);

                        return { code: "// custom output", map: null };
                    },
                },
            ],
        });

        expect(result.code).toBe("// custom output");
        expect(result.map).toBeNull();
    });

    it("passes createParser/createGenerator results through hooks and delivers output once", async (): Promise<void> => {
        const seen: Array<string> = [];

        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            sourceMap: { output: "external" },
            plugins: [
                {
                    name: "builtin-verbatim",
                    parse: async (
                        _ctx: PluginContext,
                        args: ParseArgs,
                    ): Promise<AstResult> => {
                        const parser: Parser = createParser(
                            args.code,
                            args.options,
                        );

                        seen.push("parse");

                        return await parser.parse();
                    },
                    generate: async (
                        _ctx: PluginContext,
                        args: GenerateArgs,
                    ): Promise<CodeResult> => {
                        const generator: Generator = createGenerator(
                            args.ast,
                            args.options,
                        );

                        seen.push("generate");

                        return await generator.generate();
                    },
                },
            ],
        });

        expect(seen).toEqual(["parse", "generate"]);
        expect(result.code).toBe(
            `const a = 1;\n${SOURCE_MAPPING_COMMENT}index.ts.map\n`,
        );
        expect(countSourceMappingComments(result.code)).toBe(1);
        expect(result.map?.version).toBe(3);
    });
});
