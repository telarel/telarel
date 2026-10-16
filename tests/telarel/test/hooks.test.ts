import type {
    AstResult,
    CodeResult,
    CompileResult,
    EndArgs,
    FinalizeArgs,
    ParseArgs,
    PluginContext,
    PrepareArgs,
    SourceMap,
    TransformArgs,
} from "telarel";

import { compile } from "telarel";
import { describe, expect, it } from "vitest";

type CaptureProgramOptions = {
    code: string;
};

/**
 * Capture a real ESTree program for `code` by running a pass-through compile
 * and cloning the AST the `transform` hook receives. The JS tests have no
 * parser of their own, so a `parse` hook test hands this snapshot back.
 */
const captureProgram = async ({
    code,
}: CaptureProgramOptions): Promise<unknown> => {
    let snapshot: unknown;

    await compile({
        cwd: "/repo",
        file: "index.ts",
        code,
        plugins: [
            {
                name: "snapshotter",
                transform: (_ctx: PluginContext, args: TransformArgs): void => {
                    snapshot = structuredClone(args.ast);
                },
            },
        ],
    });

    return snapshot;
};

type Program = AstResult["ast"]["program"];

describe("hooks", (): void => {
    it("parse: first non-null wins and later hooks are ignored", async (): Promise<void> => {
        const program: Program = (await captureProgram({
            code: "let custom = 1;",
        })) as Program;
        const seen: Array<string> = [];

        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                {
                    name: "parser-first",
                    parse: (
                        _ctx: PluginContext,
                        _args: ParseArgs,
                    ): AstResult => {
                        seen.push("first");

                        return {
                            ast: { program, code: "let custom = 1;" },
                        };
                    },
                },
                {
                    name: "parser-second",
                    parse: (
                        _ctx: PluginContext,
                        _args: ParseArgs,
                    ): AstResult => {
                        seen.push("second");

                        return {
                            ast: { program, code: "let custom = 1;" },
                        };
                    },
                },
            ],
        });

        expect(seen).toEqual(["first"]);
        expect(result.code).toBe("let custom = 1;");
    });

    it("parse: a null return defers to the next hook", async (): Promise<void> => {
        const program: Program = (await captureProgram({
            code: "let second = 2;",
        })) as Program;
        const seen: Array<string> = [];

        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                {
                    name: "parser-first",
                    parse: (_ctx: PluginContext, _args: ParseArgs): null => {
                        seen.push("first");

                        return null;
                    },
                },
                {
                    name: "parser-second",
                    parse: (
                        _ctx: PluginContext,
                        _args: ParseArgs,
                    ): AstResult => {
                        seen.push("second");

                        return {
                            ast: { program, code: "let second = 2;" },
                        };
                    },
                },
            ],
        });

        expect(seen).toEqual(["first", "second"]);
        expect(result.code).toBe("let second = 2;");
    });

    it("parse: all hooks deferring falls back to the builtin parser", async (): Promise<void> => {
        const seen: Array<string> = [];

        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                {
                    name: "defers",
                    parse: (_ctx: PluginContext, _args: ParseArgs): null => {
                        seen.push("defers");

                        return null;
                    },
                },
            ],
        });

        expect(seen).toEqual(["defers"]);
        expect(result.code).toBe("const a = 1;");
        expect(result.map?.version).toBe(3);
    });

    it("parse: wraps a hook error with the stage prefix", async (): Promise<void> => {
        await expect(
            compile({
                cwd: "/repo",
                file: "index.ts",
                code: "const a = 1;",
                plugins: [
                    {
                        name: "boom-parse",
                        parse: (): never => {
                            throw new Error("parse-kaboom");
                        },
                    },
                ],
            }),
        ).rejects.toThrow("parse hook: `boom-parse` parse");
    });

    it("generate: first non-null wins and later hooks are ignored", async (): Promise<void> => {
        const seen: Array<string> = [];

        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
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

        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
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

        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
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

    it("generate: wraps a hook error with the stage prefix", async (): Promise<void> => {
        await expect(
            compile({
                cwd: "/repo",
                file: "index.ts",
                code: "const a = 1;",
                plugins: [
                    {
                        name: "boom-generate",
                        generate: (): never => {
                            throw new Error("generate-kaboom");
                        },
                    },
                ],
            }),
        ).rejects.toThrow("generate hook: `boom-generate` generate");
    });

    it("prepare and finalize observe only the ast key", async (): Promise<void> => {
        const prepareKeys: Array<Array<string>> = [];
        const finalizeKeys: Array<Array<string>> = [];

        await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                {
                    name: "prepare-observer",
                    prepare: (_ctx: PluginContext, args: PrepareArgs): void => {
                        prepareKeys.push(Object.keys(args));
                    },
                },
                {
                    name: "finalize-observer",
                    finalize: (
                        _ctx: PluginContext,
                        args: FinalizeArgs,
                    ): void => {
                        finalizeKeys.push(Object.keys(args));
                    },
                },
            ],
        });

        expect(prepareKeys).toEqual([["ast"]]);
        expect(finalizeKeys).toEqual([["ast"]]);
    });

    it("end receives only the error key on the success path", async (): Promise<void> => {
        const seen: Array<{ keys: Array<string>; error: string | null }> = [];

        const result: CompileResult = await compile({
            cwd: "/repo",
            file: "index.ts",
            code: "const a = 1;",
            plugins: [
                {
                    name: "end-observer",
                    end: (_ctx: PluginContext, args: EndArgs): void => {
                        seen.push({
                            keys: Object.keys(args),
                            error: args.error,
                        });
                    },
                },
            ],
        });

        expect(result.code).toBe("const a = 1;");
        expect(seen).toEqual([{ keys: ["error"], error: null }]);
    });

    it("end receives the first stage error on failure", async (): Promise<void> => {
        const seen: Array<string> = [];

        await expect(
            compile({
                cwd: "/repo",
                file: "index.ts",
                code: "const a = 1;",
                plugins: [
                    {
                        name: "end-observer",
                        end: (_ctx: PluginContext, args: EndArgs): void => {
                            seen.push(args.error ?? "");
                        },
                    },
                    {
                        name: "boom",
                        transform: (): never => {
                            throw new Error("kaboom");
                        },
                    },
                ],
            }),
        ).rejects.toThrow("kaboom");

        expect(seen).toHaveLength(1);
        expect(seen[0]).toContain("kaboom");
        expect(seen[0]).toContain("transform hook");
    });
});
