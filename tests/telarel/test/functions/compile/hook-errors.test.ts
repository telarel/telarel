import type {
    CodeResult,
    Plugin,
    PluginContext,
    Program,
    TransformArgs,
    TransformResult,
} from "telarel";

import type { ProgramFixture } from "#/functions/ast";

import { compile } from "telarel";
import { describe, expect, it } from "vitest";

import { renameRootIdentifier } from "#/functions/ast";

describe("compile hook errors", (): void => {
    it("rejects on hook errors with plugin context", async (): Promise<void> => {
        await expect(
            compile("const a = 1;", {
                cwd: "/repo",
                file: "index.ts",
                plugins: [
                    {
                        name: "boom",
                        transform: (): never => {
                            throw new Error("kaboom");
                        },
                    },
                ],
            }),
        ).rejects.toThrow("kaboom");
    });

    it("rejects a plugin with a missing name", async (): Promise<void> => {
        const nameless: unknown = {
            prepare: (): void => {},
        };

        const build = async (): Promise<CodeResult> =>
            await compile("const a = 1;", {
                cwd: "/repo",
                file: "index.ts",
                plugins: [nameless as Plugin],
            });

        await expect(build()).rejects.toThrow(TypeError);
        await expect(build()).rejects.toThrow(
            "plugin.name must be a non-empty string",
        );
    });

    it("rejects a plugin with an empty name", async (): Promise<void> => {
        const empty: unknown = {
            name: "",
            prepare: (): void => {},
        };

        const build = async (): Promise<CodeResult> =>
            await compile("const a = 1;", {
                cwd: "/repo",
                file: "index.ts",
                plugins: [empty as Plugin],
            });

        await expect(build()).rejects.toThrow(TypeError);
        await expect(build()).rejects.toThrow(
            "plugin.name must be a non-empty string",
        );
    });

    it("rejects on an async hook rejection", async (): Promise<void> => {
        await expect(
            compile("const a = 1;", {
                cwd: "/repo",
                file: "index.ts",
                plugins: [
                    {
                        name: "boom-async",
                        transform: async (): Promise<never> => {
                            throw new Error("async-kaboom");
                        },
                    },
                ],
            }),
        ).rejects.toThrow("async-kaboom");
    });

    it("rejects on an error thrown from prepare", async (): Promise<void> => {
        await expect(
            compile("const a = 1;", {
                cwd: "/repo",
                file: "index.ts",
                plugins: [
                    {
                        name: "boom-prepare",
                        prepare: (): never => {
                            throw new Error("prepare-kaboom");
                        },
                    },
                ],
            }),
        ).rejects.toThrow("prepare-kaboom");
    });

    it("rejects on an error thrown from finalize", async (): Promise<void> => {
        await expect(
            compile("const a = 1;", {
                cwd: "/repo",
                file: "index.ts",
                plugins: [
                    {
                        name: "boom-finalize",
                        finalize: (): never => {
                            throw new Error("finalize-kaboom");
                        },
                    },
                ],
            }),
        ).rejects.toThrow("finalize-kaboom");
    });

    it("rejects on an error thrown from options", async (): Promise<void> => {
        await expect(
            compile("const a = 1;", {
                cwd: "/repo",
                file: "index.ts",
                plugins: [
                    {
                        name: "boom-options",
                        options: (): never => {
                            throw new Error("options-kaboom");
                        },
                    },
                ],
            }),
        ).rejects.toThrow("options-kaboom");
    });

    it("treats a non-function hook value as absent", async (): Promise<void> => {
        const broken: unknown = {
            name: "broken-shape",
            transform: "nope",
        };

        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [broken as Plugin],
        });

        expect(result.code).toBe("const a = 1;");
        expect(result.map?.mappings.length).toBeGreaterThan(0);
    });

    it("rejects cleanly on a non-Error throwable from transform", async (): Promise<void> => {
        await expect(
            compile("const a = 1;", {
                cwd: "/repo",
                file: "index.ts",
                plugins: [
                    {
                        name: "boom-string",
                        transform: async (): Promise<never> => {
                            // oxlint-disable-next-line typescript/only-throw-error
                            throw "boom-string";
                        },
                    },
                ],
            }),
        ).rejects.toThrow("boom-string");

        // The object payload does not reach the caller. The bridge renders any
        // non-Error throwable as `GenericFailure`, and the wrapper no longer
        // adds the plugin name.
        await expect(
            compile("const a = 1;", {
                cwd: "/repo",
                file: "index.ts",
                plugins: [
                    {
                        name: "boom-object",
                        transform: async (): Promise<never> => {
                            // oxlint-disable-next-line typescript/only-throw-error
                            throw { reason: "boom-object" };
                        },
                    },
                ],
            }),
        ).rejects.toThrow("GenericFailure");
    });

    it("rejects with the late error after a prior plugin replaced the ast", async (): Promise<void> => {
        // oxlint-disable-next-line unicorn/consistent-function-scoping
        const build = async (): Promise<CodeResult> =>
            await compile("const a = 1;", {
                cwd: "/repo",
                file: "index.ts",
                plugins: [
                    {
                        name: "rename",
                        transform: (
                            _ctx: PluginContext,
                            args: TransformArgs,
                        ): TransformResult => {
                            args.ast.mutate((program: Program): void => {
                                renameRootIdentifier({
                                    program:
                                        program as unknown as ProgramFixture,
                                    name: "renamed",
                                });
                            });

                            return { ast: args.ast };
                        },
                    },
                    {
                        name: "boom-late",
                        transform: async (): Promise<never> => {
                            throw new Error("late-kaboom");
                        },
                    },
                ],
            });

        await expect(build()).rejects.toThrow("late-kaboom");
    });
});
