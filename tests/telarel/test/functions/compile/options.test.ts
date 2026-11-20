import type {
    CodeResult,
    CommonPluginContext,
    GenerateArgs,
    OptionsArgs,
    Plugin,
    PluginContext,
    PluginOption,
    PrepareArgs,
    StartArgs,
} from "telarel";

import { compile } from "telarel";
import { describe, expect, it } from "vitest";

type DeclarationProgramFixture = {
    body: Array<{
        declarations?: Array<{
            init?: { type: string } | null;
        }>;
    }>;
};

type NamedPluginPayload = {
    name: string;
};

/**
 * Read the names of the descriptors carried by an `options` payload. The
 * payload's entries are bridge-normalized descriptors at runtime.
 */
const payloadPluginNames = (
    plugins: ReadonlyArray<PluginOption> | undefined,
): Array<string> =>
    (plugins ?? []).map(
        (plugin: PluginOption): string =>
            (plugin as unknown as NamedPluginPayload).name,
    );

describe("compile options", (): void => {
    it("chains options hooks last-wins on the file", async (): Promise<void> => {
        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "rewrite-b",
                    options: (
                        _ctx: CommonPluginContext,
                        args: OptionsArgs,
                    ): OptionsArgs | null | void => ({
                        options: {
                            ...args.options,
                            file: "b.ts",
                        },
                    }),
                },
                {
                    name: "rewrite-c",
                    options: (
                        _ctx: CommonPluginContext,
                        args: OptionsArgs,
                    ): OptionsArgs | null | void => ({
                        options: {
                            ...args.options,
                            file: "c.ts",
                        },
                    }),
                },
            ],
        });

        // The options bag cannot carry a `code` rewrite, so the source stays
        // unchanged. The last returned bag wins the file.
        expect(result.code).toBe("const a = 1;");
        expect(result.map?.sources).toEqual(["c.ts"]);
    });

    it("keeps the bag unchanged when an options hook returns null", async (): Promise<void> => {
        const seen: Array<string> = [];

        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "rewrite-b",
                    options: (
                        _ctx: CommonPluginContext,
                        args: OptionsArgs,
                    ): OptionsArgs | null | void => ({
                        options: {
                            ...args.options,
                            file: "b.ts",
                        },
                    }),
                },
                {
                    name: "passthrough",
                    options: (
                        _ctx: CommonPluginContext,
                        _args: OptionsArgs,
                    ): null => null,
                },
                {
                    name: "observer",
                    options: (
                        _ctx: CommonPluginContext,
                        args: OptionsArgs,
                    ): null => {
                        seen.push(args.options.file ?? "");

                        return null;
                    },
                },
            ],
        });

        // `null` = no change: the bag carries `b.ts` into the observer and
        // nothing after it replaces the file.
        expect(seen).toEqual(["b.ts"]);
        expect(result.code).toBe("const a = 1;");
        expect(result.map?.sources).toEqual(["b.ts"]);
    });

    it("propagates returned options bags across plugins", async (): Promise<void> => {
        // Later plugins observe the bag returned by earlier plugins.
        const seen: Array<string> = [];

        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "rewrite-cwd",
                    options: (
                        _ctx: CommonPluginContext,
                        args: OptionsArgs,
                    ): OptionsArgs | null | void => ({
                        options: {
                            ...args.options,
                            cwd: "/other",
                        },
                    }),
                },
                {
                    name: "rewrite-file",
                    options: (
                        _ctx: CommonPluginContext,
                        args: OptionsArgs,
                    ): OptionsArgs | null | void => {
                        seen.push(args.options.file ?? "");
                        seen.push(args.options.cwd ?? "");

                        return {
                            options: {
                                ...args.options,
                                file: "renamed.ts",
                            },
                        };
                    },
                },
                {
                    name: "observe",
                    prepare: (ctx: PluginContext): void => {
                        seen.push(ctx.module.file);
                    },
                },
            ],
        });

        expect(seen).toEqual(["index.ts", "/other", "renamed.ts"]);
        expect(result.code).toBe("const a = 1;");
        expect(result.map?.sources).toEqual(["renamed.ts"]);
    });

    it("merges a returned bag with the settled options", async (): Promise<void> => {
        // A returned bag is a patch. Fields the bag omits keep their settled
        // values; here it repeats `file` and `plugins`, so both are unchanged.
        const seen: Array<string> = [];

        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "replace-bag",
                    options: (
                        _ctx: CommonPluginContext,
                        args: OptionsArgs,
                    ): OptionsArgs | null | void => {
                        seen.push("has-context");
                        seen.push(args.options.file ?? "");

                        return {
                            options: {
                                file: args.options.file,
                                plugins: args.options.plugins,
                            },
                        };
                    },
                    prepare: (ctx: PluginContext): void => {
                        seen.push(ctx.module.file);
                    },
                },
            ],
        });

        expect(seen).toEqual(["has-context", "index.ts", "index.ts"]);
        expect(result.map?.sources).toEqual(["index.ts"]);
        expect(result.code).toBe("const a = 1;");
    });

    it("lets an options hook set parser and generator options", async (): Promise<void> => {
        const seenAst: Array<string> = [];
        const seenBag: Array<{
            preserveParens: boolean | void;
            singleQuote: boolean | void;
            asciiOnly: boolean | void;
        }> = [];
        const seenGenerator: Array<{
            singleQuote: boolean;
            asciiOnly: boolean;
            indentWidth: number;
        }> = [];

        const result: CodeResult = await compile(
            'const µ = "hello";\nconst b = (1);\nfunction f() {\nreturn b;\n}',
            {
                file: "index.ts",
                parser: { preserveParens: false },
                generator: {
                    singleQuote: false,
                    oxc: { asciiOnly: false },
                },
                plugins: [
                    {
                        name: "set-stage-options",
                        options: (
                            _ctx: CommonPluginContext,
                            args: OptionsArgs,
                        ): OptionsArgs => {
                            seenBag.push({
                                preserveParens:
                                    args.options.parser?.preserveParens,
                                singleQuote:
                                    args.options.generator?.singleQuote,
                                asciiOnly:
                                    args.options.generator?.oxc?.asciiOnly,
                            });

                            return {
                                options: {
                                    ...args.options,
                                    parser: {
                                        ...args.options.parser,
                                        preserveParens: true,
                                    },
                                    generator: {
                                        ...args.options.generator,
                                        singleQuote: true,
                                        oxc: { asciiOnly: true },
                                    },
                                },
                            };
                        },
                        prepare: (
                            _ctx: PluginContext,
                            args: PrepareArgs,
                        ): void => {
                            const program: DeclarationProgramFixture = args.ast
                                .program as unknown as DeclarationProgramFixture;

                            seenAst.push(
                                program.body[1]?.declarations?.[0]?.init
                                    ?.type ?? "",
                            );
                        },
                        generate: (
                            _ctx: PluginContext,
                            args: GenerateArgs,
                        ): null => {
                            seenGenerator.push({
                                singleQuote: args.options.singleQuote,
                                asciiOnly: args.options.asciiOnly,
                                indentWidth: args.options.indentWidth,
                            });

                            return null;
                        },
                    },
                ],
            },
        );

        expect(seenBag).toEqual([
            {
                preserveParens: false,
                singleQuote: false,
                asciiOnly: false,
            },
        ]);
        expect(seenAst).toEqual(["ParenthesizedExpression"]);
        expect(seenGenerator).toEqual([
            { singleQuote: true, asciiOnly: true, indentWidth: 2 },
        ]);
        expect(result.code).toContain("const \\u00B5 = 'hello';");
        expect(result.code).toContain("const b = 1;");
        expect(result.code).toContain("function f() {\n  return b;\n}");
    });

    it("omits unset cwd and file from the options hook and resolves their defaults", async (): Promise<void> => {
        // The `options` fixpoint runs before resolution, so the options bag
        // has no `cwd` or `file` when the caller omits them. If an unset field
        // carried `""`, echoing the bag would shadow the defaults. The core
        // resolves both fields for `start` and later hooks. `cwd` depends on
        // the runtime: the native binding reports Node's `process.cwd()`, and
        // the WASI runtime starts at `/`. Pin the default for the active
        // runtime. To select WASI, set `NAPI_RS_FORCE_WASI=error`.
        const isWasi: boolean = process.env.NAPI_RS_FORCE_WASI === "error";

        const expectedCwd: string = isWasi ? "/" : process.cwd();

        const seenRaw: Array<{
            hasCwd: boolean;
            hasFile: boolean;
            cwd: string | void;
            file: string | void;
        }> = [];

        const seenResolved: Array<{ cwd: string; file: string }> = [];

        const result: CodeResult = await compile("const a = 1;", {
            plugins: [
                {
                    name: "observe-raw-options",
                    options: (
                        _ctx: CommonPluginContext,
                        args: OptionsArgs,
                    ): null => {
                        seenRaw.push({
                            hasCwd: "cwd" in args.options,
                            hasFile: "file" in args.options,
                            cwd: args.options.cwd,
                            file: args.options.file,
                        });

                        return null;
                    },
                },
                {
                    name: "observe-resolved-options",
                    start: (_ctx: PluginContext, args: StartArgs): void => {
                        seenResolved.push({
                            cwd: args.options.cwd,
                            file: args.options.file,
                        });
                    },
                },
            ],
        });

        expect(seenRaw).toEqual([
            {
                hasCwd: false,
                hasFile: false,
                cwd: void 0,
                file: void 0,
            },
        ]);
        expect(seenResolved).toEqual([{ cwd: expectedCwd, file: "index.ts" }]);
        expect(result.code).toBe("const a = 1;");
    });

    it("omits absent parser and generator groups from the options hook", async (): Promise<void> => {
        // An absent group means unset, so resolution applies the stage
        // defaults. An echoed empty group must not shadow them.
        const seen: Array<{
            hasParser: boolean;
            hasGenerator: boolean;
            parser: unknown;
            generator: unknown;
        }> = [];

        const result: CodeResult = await compile("const a = 1;", {
            plugins: [
                {
                    name: "observe-absent-groups",
                    options: (
                        _ctx: CommonPluginContext,
                        args: OptionsArgs,
                    ): null => {
                        seen.push({
                            hasParser: "parser" in args.options,
                            hasGenerator: "generator" in args.options,
                            parser: args.options.parser,
                            generator: args.options.generator,
                        });

                        return null;
                    },
                },
            ],
        });

        expect(seen).toEqual([
            {
                hasParser: false,
                hasGenerator: false,
                parser: void 0,
                generator: void 0,
            },
        ]);
        expect(result.code).toBe("const a = 1;");
    });

    it("preserves defaults when an options hook echoes an unset bag", async (): Promise<void> => {
        // Regression guard: unset fields stay absent from the raw bag. An
        // echoed bag keeps the `index.ts` default and the ts grammar.
        const isWasi: boolean = process.env.NAPI_RS_FORCE_WASI === "error";
        const expectedCwd: string = isWasi ? "/" : process.cwd();
        const seen: Array<{ cwd: string; file: string }> = [];

        const result: CodeResult = await compile("const a: number = 1;", {
            plugins: [
                {
                    name: "echo-options",
                    options: (
                        _ctx: CommonPluginContext,
                        args: OptionsArgs,
                    ): OptionsArgs => ({
                        options: { ...args.options },
                    }),
                },
                {
                    name: "observe-resolved-options",
                    start: (_ctx: PluginContext, args: StartArgs): void => {
                        seen.push({
                            cwd: args.options.cwd,
                            file: args.options.file,
                        });
                    },
                },
            ],
        });

        expect(seen).toEqual([{ cwd: expectedCwd, file: "index.ts" }]);
        expect(result.map?.sources).toEqual(["index.ts"]);
        expect(result.code).toBe("const a: number = 1;");
    });

    it("keeps the settled cwd when a late options hook omits it", async (): Promise<void> => {
        const seen: Array<string> = [];

        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "drop-cwd",
                    options: (
                        _ctx: CommonPluginContext,
                        args: OptionsArgs,
                    ): OptionsArgs => {
                        return {
                            options: { ...args.options, cwd: void 0 },
                        };
                    },
                },
                {
                    name: "observe-start-cwd",
                    start: (_ctx: PluginContext, args: StartArgs): void => {
                        seen.push(args.options.cwd);
                    },
                },
            ],
        });

        // The returned bag is a patch. An absent `cwd` keeps the settled
        // `/repo`; it does not fall back to the process working directory.
        expect(seen).toEqual(["/repo"]);
        expect(result.code).toBe("const a = 1;");
    });

    it("awaits an async options hook and applies the returned bag", async (): Promise<void> => {
        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "async-options",
                    options: async (
                        _ctx: CommonPluginContext,
                        args: OptionsArgs,
                    ): Promise<OptionsArgs> => {
                        await new Promise<void>((resolve): void => {
                            setTimeout(resolve, 0);
                        });

                        return {
                            options: {
                                ...args.options,
                                file: "async.ts",
                            },
                        };
                    },
                },
            ],
        });

        expect(result.code).toBe("const a = 1;");
        expect(result.map?.sources).toEqual(["async.ts"]);
    });

    it("injects a plugin through a returned options bag", async (): Promise<void> => {
        // An options hook patches the bag; returned plugins append. The
        // bridge wraps the injected plugin. Then the plugin joins the settled
        // list and runs like any entry plugin. The injection appends only
        // once, so the fixpoint converges.
        let isInjected: boolean = false;
        const seen: Array<string> = [];

        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "injector",
                    options: (
                        _ctx: CommonPluginContext,
                        args: OptionsArgs,
                    ): OptionsArgs | null | void => {
                        if (isInjected) {
                            return null;
                        }
                        isInjected = true;

                        return {
                            options: {
                                ...args.options,
                                plugins: [
                                    ...(args.options.plugins ?? []),
                                    {
                                        name: "injected",
                                        prepare: (): void => {
                                            seen.push("injected.prepare");
                                        },
                                    },
                                ],
                            },
                        };
                    },
                },
            ],
        });

        expect(seen).toEqual(["injected.prepare"]);
        expect(result.code).toBe("const a = 1;");
    });

    it("runs the injected plugin's options hook in the next fixpoint pass", async (): Promise<void> => {
        let isInjected: boolean = false;
        let lateRan: boolean = false;
        const seen: Array<string> = [];

        await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "injector-observer",
                    options: (
                        _ctx: CommonPluginContext,
                        args: OptionsArgs,
                    ): OptionsArgs | null | void => {
                        seen.push(`in:${String(lateRan)}`);

                        if (isInjected) {
                            return null;
                        }
                        isInjected = true;

                        const next: OptionsArgs = {
                            options: {
                                ...args.options,
                                plugins: [
                                    ...(args.options.plugins ?? []),
                                    {
                                        name: "late",
                                        options: (
                                            _lateCtx: CommonPluginContext,
                                            _lateArgs: OptionsArgs,
                                        ): void => {
                                            lateRan = true;
                                            seen.push("late.options");
                                        },
                                    },
                                ],
                            },
                        };

                        return next;
                    },
                },
            ],
        });

        // The injector runs once. `late` joins on the next pass, after the
        // fixpoint skips the injector as an already-run instance.
        expect(seen).toEqual(["in:false", "late.options"]);
    });

    it("runs an appended same-name plugin's options hook on the next pass", async (): Promise<void> => {
        let isSwapped: boolean = false;
        const seen: Array<string> = [];

        const oldProbe: Plugin = {
            name: "same",
            options: (): void => {
                seen.push("old.options");
            },
            start: (): void => {
                seen.push("old.start");
            },
        };

        const newProbe: Plugin = {
            name: "same",
            options: (): void => {
                seen.push("new.options");
            },
            start: (): void => {
                seen.push("new.start");
            },
        };

        const swapper: Plugin = {
            name: "swapper",
            options: (
                _ctx: CommonPluginContext,
                args: OptionsArgs,
            ): OptionsArgs | null | void => {
                seen.push("swapper.options");

                if (isSwapped) {
                    return null;
                }
                isSwapped = true;

                return {
                    options: {
                        ...args.options,
                        plugins: [swapper, newProbe],
                    },
                };
            },
        };

        await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [swapper, oldProbe],
        });

        // The fixpoint treats the same-name instance as new work on the next
        // pass. A returned plugin list is a patch: `newProbe` appends after
        // `oldProbe`, so the settled list keeps both instances and both run
        // `start`.
        expect(seen).toEqual([
            "swapper.options",
            "old.options",
            "new.options",
            "old.start",
            "new.start",
        ]);
    });

    it("does not re-run options when the original plugin instance is returned again", async (): Promise<void> => {
        let runs: number = 0;

        const self: Plugin = {
            name: "self",
            options: (
                _ctx: CommonPluginContext,
                args: OptionsArgs,
            ): OptionsArgs | null | void => {
                runs++;

                if (runs > 1) {
                    return null;
                }

                return {
                    options: {
                        ...args.options,
                        plugins: [...(args.options.plugins ?? []), self],
                    },
                };
            },
        };

        await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [self],
        });

        expect(runs).toBe(1);
    });

    it("runs options once for duplicate entries of the same instance", async (): Promise<void> => {
        let runs: number = 0;
        const seen: Array<Array<string>> = [];

        const plugin: Plugin = {
            name: "dup",
            options: (): void => {
                runs++;
            },
            start: (_ctx: PluginContext, args: StartArgs): void => {
                seen.push(args.options.plugins);
            },
        };

        await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [plugin, plugin],
        });

        expect(runs).toBe(1);
        expect(seen).toEqual([
            ["dup", "dup"],
            ["dup", "dup"],
        ]);
    });

    it("runs options for distinct same-name plugin instances", async (): Promise<void> => {
        const seen: Array<string> = [];

        const first: Plugin = {
            name: "same",
            options: (): void => {
                seen.push("first.options");
            },
        };

        const second: Plugin = {
            name: "same",
            options: (): void => {
                seen.push("second.options");
            },
        };

        await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [first, second],
        });

        expect(seen).toEqual(["first.options", "second.options"]);
    });

    it("exposes the settled plugin list to a later options hook after a scalar patch", async (): Promise<void> => {
        const seen: Array<Array<string>> = [];

        const patchFile: Plugin = {
            name: "a",
            options: (
                _ctx: CommonPluginContext,
                args: OptionsArgs,
            ): OptionsArgs => ({
                options: { ...args.options, file: "a.ts" },
            }),
        };

        const observePlugins: Plugin = {
            name: "b",
            options: (_ctx: CommonPluginContext, args: OptionsArgs): null => {
                seen.push(payloadPluginNames(args.options.plugins));

                return null;
            },
        };

        await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [patchFile, observePlugins],
        });

        // The scalar-only patch appends no descriptors, so the later hook still
        // observes the full entry list.
        expect(seen).toEqual([["a", "b"]]);
    });

    it("exposes an added plugin to later options hooks with the full settled list", async (): Promise<void> => {
        const seen: Array<string> = [];

        const added: Plugin = {
            name: "c",
            options: (_ctx: CommonPluginContext, args: OptionsArgs): null => {
                seen.push(
                    `c sees: ${payloadPluginNames(args.options.plugins).join(",")}`,
                );

                return null;
            },
        };

        const adder: Plugin = {
            name: "a",
            options: (
                _ctx: CommonPluginContext,
                args: OptionsArgs,
            ): OptionsArgs => ({
                options: {
                    ...args.options,
                    plugins: [...(args.options.plugins ?? []), added],
                },
            }),
        };

        const observer: Plugin = {
            name: "b",
            options: (_ctx: CommonPluginContext, args: OptionsArgs): null => {
                seen.push(
                    `b sees: ${payloadPluginNames(args.options.plugins).join(",")}`,
                );

                return null;
            },
        };

        await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [adder, observer],
        });

        // The added plugin appends before the later hook runs, and it observes
        // the same settled list on the next pass.
        expect(seen).toEqual(["b sees: a,b,c", "c sees: a,b,c"]);
    });

    it("applies an in-place curated group-field change", async (): Promise<void> => {
        const seen: Array<string> = [];

        const mutator: Plugin = {
            name: "mutator",
            options: (
                _ctx: CommonPluginContext,
                args: OptionsArgs,
            ): OptionsArgs => {
                if (args.options.output !== void 0) {
                    args.options.output.file = "mutated.js";
                }

                return { options: args.options };
            },
        };

        const observer: Plugin = {
            name: "observer",
            start: (_ctx: PluginContext, args: StartArgs): void => {
                seen.push(args.options.output.file);
            },
        };

        await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            output: { file: "orig.js" },
            plugins: [mutator, observer],
        });

        expect(seen).toEqual(["mutated.js"]);
    });
});
