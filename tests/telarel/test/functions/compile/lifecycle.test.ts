import type {
    CodeResult,
    CommonPluginContext,
    EndArgs,
    FinalizeArgs,
    Language,
    OptionsArgs,
    Plugin,
    PluginContext,
    PrepareArgs,
    Program,
    ResolvedOptions,
    SourceType,
    StartArgs,
    TransformArgs,
    TransformResult,
} from "telarel";

import type { ProgramFixture } from "#/functions/ast";

import { compile } from "telarel";
import { describe, expect, it } from "vitest";

import { renameRootIdentifier } from "#/functions/ast";

describe("compile lifecycle", (): void => {
    it("runs async prepare, transform, and finalize hooks", async (): Promise<void> => {
        const seen: Array<string> = [];

        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "async-all",
                    options: (
                        _ctx: CommonPluginContext,
                        args: OptionsArgs,
                    ): OptionsArgs | null | void => {
                        seen.push("options");

                        return {
                            options: {
                                ...args.options,
                                file: "b.ts",
                            },
                        };
                    },
                    prepare: async (): Promise<void> => {
                        seen.push("prepare");
                    },
                    transform: async (): Promise<void> => {
                        seen.push("transform");
                    },
                    finalize: async (): Promise<void> => {
                        seen.push("finalize");
                    },
                },
            ],
        });

        expect(result.code).toBe("const a = 1;");
        expect(result.map?.sources).toEqual(["b.ts"]);
        expect(seen).toEqual(["options", "prepare", "transform", "finalize"]);
    });

    it("orders prepare, transform, and finalize across plugins", async (): Promise<void> => {
        const seen: Array<string> = [];

        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "first",
                    prepare: (): void => {
                        seen.push("first.prepare");
                    },
                    transform: (_ctx: PluginContext): void => {
                        seen.push("first.transform");
                    },
                    finalize: (): void => {
                        seen.push("first.finalize");
                    },
                },
                {
                    name: "second",
                    prepare: (): void => {
                        seen.push("second.prepare");
                    },
                    transform: (_ctx: PluginContext): void => {
                        seen.push("second.transform");
                    },
                    finalize: (): void => {
                        seen.push("second.finalize");
                    },
                },
            ],
        });

        expect(result.code).toBe("const a = 1;");
        expect(seen).toEqual([
            "first.prepare",
            "second.prepare",
            "first.transform",
            "second.transform",
            "first.finalize",
            "second.finalize",
        ]);
    });

    it("chains transforms return value to return value across plugins", async (): Promise<void> => {
        // oxlint-disable-next-line unicorn/consistent-function-scoping
        const rename = (
            name: string,
        ): ((ctx: PluginContext, args: TransformArgs) => TransformResult) => {
            return (
                _ctx: PluginContext,
                args: TransformArgs,
            ): TransformResult => {
                const program: ProgramFixture = args.ast
                    .program as unknown as ProgramFixture;
                if (
                    name === "second" &&
                    program.body[0]?.declarations[0]?.id.name !== "first"
                ) {
                    throw new Error("expected the first rename to be visible");
                }
                args.ast.mutate((mutable: Program): void => {
                    renameRootIdentifier({
                        program: mutable as unknown as ProgramFixture,
                        name,
                    });
                });

                return { ast: args.ast };
            };
        };

        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                { name: "rename-a", transform: rename("first") },
                { name: "rename-b", transform: rename("second") },
            ],
        });

        expect(result.code).toBe("const second = 1;");
    });

    it("passes hook contexts with cwd and module but no code", async (): Promise<void> => {
        const seen: Array<{ cwd: string; file: string; hasCode: boolean }> = [];

        const record = (ctx: PluginContext): void => {
            seen.push({
                cwd: ctx.cwd,
                file: ctx.module.file,
                hasCode: Object.prototype.hasOwnProperty.call(
                    ctx.module,
                    "code",
                ),
            });
        };

        const first: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "fidelity",
                    prepare: (ctx: PluginContext, _args: PrepareArgs): void => {
                        record(ctx);
                    },
                    transform: (ctx: PluginContext): void => {
                        record(ctx);
                    },
                    finalize: (
                        ctx: PluginContext,
                        _args: FinalizeArgs,
                    ): void => {
                        record(ctx);
                    },
                },
            ],
        });

        expect(first.code).toBe("const a = 1;");
        expect(seen).toEqual([
            { cwd: "/repo", file: "index.ts", hasCode: false },
            { cwd: "/repo", file: "index.ts", hasCode: false },
            { cwd: "/repo", file: "index.ts", hasCode: false },
        ]);

        const second: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "options-rewrite",
                    options: (
                        _ctx: CommonPluginContext,
                        args: OptionsArgs,
                    ): OptionsArgs | null | void => ({
                        options: {
                            ...args.options,
                            file: "renamed.ts",
                        },
                    }),
                },
                {
                    name: "fidelity",
                    prepare: (ctx: PluginContext, _args: PrepareArgs): void => {
                        record(ctx);
                    },
                    transform: (ctx: PluginContext): void => {
                        record(ctx);
                    },
                    finalize: (
                        ctx: PluginContext,
                        _args: FinalizeArgs,
                    ): void => {
                        record(ctx);
                    },
                },
            ],
        });

        expect(second.code).toBe("const a = 1;");
        expect(seen.slice(3)).toEqual([
            { cwd: "/repo", file: "renamed.ts", hasCode: false },
            { cwd: "/repo", file: "renamed.ts", hasCode: false },
            { cwd: "/repo", file: "renamed.ts", hasCode: false },
        ]);
    });

    it("always resolves the module info in every hook context", async (): Promise<void> => {
        const seen: Array<{
            hook: string;
            language: Language;
            sourceType: SourceType;
        }> = [];

        const record = (hook: string, ctx: PluginContext): void => {
            seen.push({
                hook,
                language: ctx.module.language,
                sourceType: ctx.module.sourceType,
            });
        };

        await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            sourceType: "module",
            plugins: [
                {
                    name: "observe-context",
                    start: (ctx: PluginContext): void => {
                        record("start", ctx);
                    },
                    parse: (ctx: PluginContext): void => {
                        record("parse", ctx);
                    },
                    prepare: (ctx: PluginContext): void => {
                        record("prepare", ctx);
                    },
                    transform: (ctx: PluginContext): void => {
                        record("transform", ctx);
                    },
                    finalize: (ctx: PluginContext): void => {
                        record("finalize", ctx);
                    },
                    generate: (ctx: PluginContext): void => {
                        record("generate", ctx);
                    },
                    end: (ctx: PluginContext): void => {
                        record("end", ctx);
                    },
                },
            ],
        });

        expect(seen).toEqual([
            { hook: "start", language: "ts", sourceType: "module" },
            { hook: "parse", language: "ts", sourceType: "module" },
            { hook: "prepare", language: "ts", sourceType: "module" },
            { hook: "transform", language: "ts", sourceType: "module" },
            { hook: "finalize", language: "ts", sourceType: "module" },
            { hook: "generate", language: "ts", sourceType: "module" },
            { hook: "end", language: "ts", sourceType: "module" },
        ]);
    });

    it("invokes start with the resolved options", async (): Promise<void> => {
        const seen: Array<ResolvedOptions> = [];
        const seenFiles: Array<string> = [];

        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "first",
                    prepare: (): void => void 0,
                },
                {
                    name: "observe",
                    transform: (): void => void 0,
                    start: (ctx: PluginContext, args: StartArgs): void => {
                        seen.push(args.options);
                        seenFiles.push(ctx.module.file);
                    },
                },
            ],
        });

        expect(result.code).toBe("const a = 1;");
        expect(seen).toHaveLength(1);
        expect(seenFiles).toEqual(["index.ts"]);
        // The core fully resolves each scalar field. It infers `language`
        // from the extension and resolves `sourceType` from the language.
        const options: ResolvedOptions = seen[0] as ResolvedOptions;
        expect(options.language).toBe("ts");
        expect(options.sourceType).toBe("unambiguous");
        expect(options.file).toBe("index.ts");
        expect(options.sourceMap).toEqual({ input: null, output: null });
        expect(options.parser).toEqual({
            preserveParens: false,
            allowReturnOutsideFunction: false,
            oxc: {
                parseRegularExpression: false,
                allowReturnOutsideFunction: false,
                preserveParens: false,
                allowV8Intrinsics: false,
                enableIdentHashes: true,
            },
        });
        expect(options.generator).toEqual({
            singleQuote: false,
            asciiOnly: false,
            indentStyle: "space",
            indentWidth: 2,
            comments: true,
            minify: false,
            oxc: {
                singleQuote: false,
                minify: false,
                asciiOnly: false,
                comments: {
                    normal: true,
                    jsdoc: true,
                    annotation: true,
                    legal: "inline",
                },
                indentChar: "space",
                indentWidth: 2,
                initialIndent: 0,
            },
        });
        // The settled plugin-name list preserves order and duplicates.
        expect(options.plugins).toEqual(["first", "observe"]);
    });

    it("resolves the defaulted cwd for start", async (): Promise<void> => {
        const isWasi: boolean = process.env.NAPI_RS_FORCE_WASI === "error";
        const expected: string = isWasi ? "/" : process.cwd();
        const seen: Array<string> = [];

        await compile("const a = 1;", {
            file: "index.ts",
            plugins: [
                {
                    name: "observe",
                    start: (_ctx: PluginContext, args: StartArgs): void => {
                        seen.push(args.options.cwd);
                    },
                },
            ],
        });

        expect(seen).toEqual([expected]);
    });

    it("passes the compile context to start with the resolved module info", async (): Promise<void> => {
        const seen: Array<{
            cwd: string;
            language: string;
            sourceType: string;
        }> = [];

        await compile("module.exports = 1;", {
            cwd: "/repo",
            file: "index.cjs",
            plugins: [
                {
                    name: "observe",
                    start: (ctx: PluginContext): void => {
                        seen.push({
                            cwd: ctx.cwd,
                            language: ctx.module.language,
                            sourceType: ctx.module.sourceType,
                        });
                    },
                },
            ],
        });

        expect(seen).toEqual([
            { cwd: "/repo", language: "js", sourceType: "commonjs" },
        ]);
    });

    it("ignores a garbage return value from start", async (): Promise<void> => {
        const seen: Array<string> = [];

        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "notify",
                    start: (
                        _ctx: PluginContext,
                        _args: StartArgs,
                    ): void | Promise<void> => {
                        seen.push("started");

                        return void 0;
                    },
                },
            ],
        });

        expect(seen).toEqual(["started"]);
        expect(result.code).toBe("const a = 1;");
        expect(result.map?.sources).toEqual(["index.ts"]);
    });

    it("invokes end on the success path with a null error", async (): Promise<void> => {
        const seen: Array<string | null> = [];
        const seenFiles: Array<string> = [];

        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "observe",
                    end: (ctx: PluginContext, args: EndArgs): void => {
                        seen.push(args.error);
                        seenFiles.push(ctx.module.file);
                    },
                },
            ],
        });

        expect(result.code).toBe("const a = 1;");
        expect(seen).toEqual([null]);
        expect(seenFiles).toEqual(["index.ts"]);
    });

    it("invokes end on the error path with the first stage error", async (): Promise<void> => {
        const seen: Array<string | null> = [];

        const build = async (): Promise<CodeResult> =>
            await compile("const a = 1;", {
                cwd: "/repo",
                file: "index.ts",
                plugins: [
                    {
                        name: "observer",
                        end: (_ctx: PluginContext, args: EndArgs): void => {
                            seen.push(args.error);
                        },
                    },
                    {
                        name: "boom",
                        transform: (): never => {
                            throw new Error("kaboom");
                        },
                    },
                ],
            });

        await expect(build()).rejects.toThrow("kaboom");

        // `end` always runs: on the error path it carries the first stage
        // error, and nothing else.
        expect(seen).toHaveLength(1);
        expect(seen[0]).toContain("kaboom");
    });

    it("observes the parsed AST in prepare", async (): Promise<void> => {
        const seen: Array<{
            keys: Array<string>;
            type: string;
            code: string;
        }> = [];

        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "prepare-observer",
                    prepare: (_ctx: PluginContext, args: PrepareArgs): void => {
                        seen.push({
                            keys: Object.keys(args),
                            type: args.ast.program.type,
                            code: args.ast.source,
                        });
                    },
                },
            ],
        });

        // The prepare payload carries only the parsed AST and has no rewrite
        // channel, so the pipeline ignores the return value.
        expect(seen).toEqual([
            { keys: ["ast"], type: "Program", code: "const a = 1;" },
        ]);
        expect(result.code).toBe("const a = 1;");
    });

    it("ignores a return value from a prepare hook", async (): Promise<void> => {
        const plugin: unknown = {
            name: "prepare-ignored",
            prepare: (): unknown => ({ code: "const b = 2;" }),
        };

        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [plugin as Plugin],
        });

        expect(result.code).toBe("const a = 1;");
    });

    it("keeps the source when a prepare hook returns nothing", async (): Promise<void> => {
        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "prepare-void",
                    prepare: (): void => void 0,
                },
            ],
        });

        expect(result.code).toBe("const a = 1;");
    });

    it("observes the final AST in finalize", async (): Promise<void> => {
        const seen: Array<{
            keys: Array<string>;
            type: string;
            code: string;
        }> = [];

        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "finalize-observer",
                    finalize: (
                        _ctx: PluginContext,
                        args: FinalizeArgs,
                    ): void => {
                        seen.push({
                            keys: Object.keys(args),
                            type: args.ast.program.type,
                            code: args.ast.source,
                        });
                    },
                },
            ],
        });

        expect(seen).toEqual([
            { keys: ["ast"], type: "Program", code: "const a = 1;" },
        ]);
        expect(result.code).toBe("const a = 1;");
    });

    it("carries a transform rewrite into the AST observed by finalize", async (): Promise<void> => {
        const names: Array<string> = [];

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
                                name: "renamed",
                            });
                        });

                        return { ast: args.ast };
                    },
                },
                {
                    name: "finalize-observer",
                    finalize: (
                        _ctx: PluginContext,
                        args: FinalizeArgs,
                    ): void => {
                        const program: ProgramFixture = args.ast
                            .program as unknown as ProgramFixture;

                        names.push(
                            program.body[0]?.declarations[0]?.id.name ?? "",
                        );
                    },
                },
            ],
        });

        expect(names).toEqual(["renamed"]);
        expect(result.code).toBe("const renamed = 1;");
    });

    it("keeps the codegen map through prepare and finalize observers", async (): Promise<void> => {
        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                { name: "prepare", prepare: (): void => void 0 },
                { name: "finalize", finalize: (): void => void 0 },
            ],
        });

        expect(result.code).toBe("const a = 1;");
        expect(result.map?.version).toBe(3);
        expect(result.map?.mappings).toBe("AAAA,MAAM,IAAI");
    });
});
