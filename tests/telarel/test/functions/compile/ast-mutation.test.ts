import type {
    CodeResult,
    PluginContext,
    Program,
    TransformArgs,
    TransformResult,
} from "telarel";

import type { ProgramFixture } from "#/functions/ast";

import { Ast, compile } from "telarel";
import { walk } from "telarel/walker";
import { describe, expect, it } from "vitest";

import { renameRootIdentifier } from "#/functions/ast";

type InitFixture = {
    body: Array<{
        declarations: Array<{
            init: { type: string; value: string };
        }>;
    }>;
};

describe("compile AST mutation", (): void => {
    it("applies a returned ast mutation through json", async (): Promise<void> => {
        const result: CodeResult = await compile(`const a = "old";`, {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "nested-mutator",
                    transform: (
                        _ctx: PluginContext,
                        args: TransformArgs,
                    ): TransformResult => {
                        args.ast.mutate((program: Program): void => {
                            const fixture: InitFixture =
                                program as unknown as InitFixture;
                            const statement:
                                | InitFixture["body"][number]
                                | undefined = fixture.body[0];
                            const declaration:
                                | InitFixture["body"][number]["declarations"][number]
                                | undefined = statement?.declarations[0];
                            if (declaration?.init.type !== "Literal") {
                                throw new Error(
                                    "expected a literal initializer",
                                );
                            }
                            declaration.init.value = "new";
                        });

                        return { ast: args.ast };
                    },
                },
            ],
        });

        expect(result.code).toContain(`"new"`);
        expect(result.code).not.toContain(`"old"`);
        expect(result.map?.mappings.length).toBeGreaterThan(0);
    });

    it("applies an in-place mutation when the mutated tree is returned", async (): Promise<void> => {
        // Return-based contract: an in-place mutation of `args.ast` reaches
        // Rust only when the hook returns the mutated tree.
        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "mutate-and-return",
                    transform: (
                        _ctx: PluginContext,
                        args: TransformArgs,
                    ): TransformResult => {
                        args.ast.mutate((program: Program): void => {
                            renameRootIdentifier({
                                program: program as unknown as ProgramFixture,
                                name: "mutated",
                            });
                        });

                        return { ast: args.ast };
                    },
                },
            ],
        });

        expect(result.code).toBe("const mutated = 1;");
    });

    it("treats an in-place mutation without return as a no-op", async (): Promise<void> => {
        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "mutate-in-place",
                    transform: (
                        _ctx: PluginContext,
                        args: TransformArgs,
                    ): void => {
                        const program: ProgramFixture = args.ast
                            .program as unknown as ProgramFixture;
                        renameRootIdentifier({ program, name: "not-applied" });
                    },
                },
            ],
        });

        expect(result.code).toBe("const a = 1;");
    });

    it("applies a whole-root replacement returned from transform", async (): Promise<void> => {
        // The returned `{ ast }` is the replacement: a plugin can build a
        // fresh tree (clone-plus-edit) and return it instead of mutating.
        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "root-replace",
                    transform: (
                        _ctx: PluginContext,
                        args: TransformArgs,
                    ): TransformResult => {
                        const program: ProgramFixture = structuredClone(
                            args.ast.program,
                        ) as unknown as ProgramFixture;
                        renameRootIdentifier({ program, name: "replaced" });

                        return {
                            ast: new Ast({
                                program: program as never,
                                source: args.ast.source,
                            }),
                        };
                    },
                },
            ],
        });

        expect(result.code).toBe("const replaced = 1;");
    });

    it("applies a returned ast to the compile", async (): Promise<void> => {
        // The hook returns a new tree and does not mutate `args.ast`. Rust
        // reads back the returned tree.
        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "returned-tree",
                    transform: (
                        _ctx: PluginContext,
                        args: TransformArgs,
                    ): TransformResult => {
                        const program: ProgramFixture = structuredClone(
                            args.ast.program,
                        ) as unknown as ProgramFixture;
                        renameRootIdentifier({ program, name: "returned" });

                        return {
                            ast: new Ast({
                                program: program as never,
                                source: args.ast.source,
                            }),
                        };
                    },
                },
            ],
        });

        expect(result.code).toBe("const returned = 1;");
        expect(result.map?.version).toBe(3);
        expect(result.map?.mappings.length).toBeGreaterThan(0);
    });

    it("walks and mutates the ast via telarel/walker", async (): Promise<void> => {
        const result: CodeResult = await compile(
            "const a = <Button>hi</Button>;",
            {
                cwd: "/repo",
                file: "index.tsx",
                plugins: [
                    {
                        name: "walker-rename",
                        transform: (
                            _ctx: PluginContext,
                            args: TransformArgs,
                        ): TransformResult => {
                            args.ast.mutate((program): void => {
                                walk(program, {
                                    enter(node): void {
                                        if (
                                            node.type === "JSXIdentifier" &&
                                            node.name === "Button"
                                        ) {
                                            node.name = "Buttonx";
                                        }
                                    },
                                });
                            });

                            return { ast: args.ast };
                        },
                    },
                ],
            },
        );

        expect(result.code).toContain("<Buttonx>");
    });

    it("mutates the carried ast through Ast.mutate", async (): Promise<void> => {
        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "mutate-rename",
                    transform: (
                        _ctx: PluginContext,
                        args: TransformArgs,
                    ): TransformResult => {
                        args.ast.mutate((program): void => {
                            renameRootIdentifier({
                                program: program as unknown as ProgramFixture,
                                name: "mutated",
                            });
                        });

                        return { ast: args.ast };
                    },
                },
            ],
        });

        expect(result.code).toBe("const mutated = 1;");
    });
});
