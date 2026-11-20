import type {
    BuiltinPlugin,
    CodeResult,
    Plugin,
    PluginContext,
    TransformArgs,
} from "telarel";

import { compile } from "telarel";
import { transform } from "telarel/plugins/transform";
import { describe, expect, it } from "vitest";

const myPlugins = (onPrepare: () => void): Array<Plugin> => [
    {
        name: "writer",
        prepare: (): void => {
            onPrepare();
        },
    },
];

describe("transform builtin", (): void => {
    it("treats an unbranded plugin named builtin:* as a js plugin", async (): Promise<void> => {
        const plugin: unknown = { name: "builtin:custom" };

        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [plugin as BuiltinPlugin],
        });

        expect(result.code).toBe("const a = 1;");
    });

    it("runs alongside plain js plugins in one array", async (): Promise<void> => {
        const seen: Array<unknown> = [];
        let marker: unknown = "__NO_MARKER__";

        const jsPlugin: Plugin = {
            name: "observer",
            transform: (): void => {
                seen.push(marker);
            },
        };

        const writer: Plugin = {
            name: "writer",
            prepare: (): void => {
                marker = "from-writer";
            },
        };

        const result: CodeResult = await compile("const a: number = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [writer, transform(), jsPlugin],
        });

        expect(result.code).not.toContain(": number");
        expect(seen).toEqual(["from-writer"]);
    });

    it("composes a plugin array next to the builtin transform", async (): Promise<void> => {
        const seen: Array<unknown> = [];
        let marker: unknown = "__NO_MARKER__";

        const observer: Plugin = {
            name: "observer",
            transform: (): void => {
                seen.push(marker);
            },
        };

        const result: CodeResult = await compile("const a: number = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                myPlugins((): void => {
                    marker = "from-array";
                }),
                transform(),
                observer,
            ],
        });

        expect(result.code).not.toContain(": number");
        expect(seen).toEqual(["from-array"]);
    });

    it("gives a plain js transform hook the post-builtin ast", async (): Promise<void> => {
        const seen: Array<unknown> = [];
        let firstNode: unknown = "__NO_NODE__";

        const jsPlugin: Plugin = {
            name: "observer",
            transform: (_ctx: PluginContext, args: TransformArgs): void => {
                const body: ReadonlyArray<{ type: string }> = args.ast.program
                    .body as ReadonlyArray<{
                    type: string;
                }>;

                firstNode = body[0]?.type ?? "missing";
            },
            finalize: (): void => {
                seen.push(firstNode);
            },
        };

        const result: CodeResult = await compile("const a: number = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [transform(), jsPlugin],
        });

        expect(result.code).not.toContain(": number");
        expect(seen).toEqual(["VariableDeclaration"]);
    });

    it("accepts a hand-written marker with __builtin true", async (): Promise<void> => {
        const marker: unknown = {
            __builtin: true,
            name: "builtin:transform",
            options: { targets: ["nonsense"] },
        };

        const build = async (): Promise<CodeResult> =>
            await compile("const a = 1;", {
                cwd: "/repo",
                file: "index.ts",
                plugins: [marker as BuiltinPlugin],
            });

        await expect(build()).rejects.toThrow("nonsense");
    });

    it("rejects an unknown builtin name", async (): Promise<void> => {
        const marker: unknown = {
            __builtin: true,
            name: "builtin:nope",
        };

        const build = async (): Promise<CodeResult> =>
            await compile("const a = 1;", {
                cwd: "/repo",
                file: "index.ts",
                plugins: [marker as BuiltinPlugin],
            });

        const promise: Promise<CodeResult> = build();

        await expect(promise).rejects.toThrow(/builtin/);
        await expect(promise).rejects.toThrow("builtin:nope");
    });

    it("treats a js plugin named builtin:* with hooks as a js plugin", async (): Promise<void> => {
        const seen: Array<unknown> = [];
        let marker: unknown = "__NO_MARKER__";

        const plugin: Plugin = {
            name: "builtin:custom",
            prepare: (): void => {
                marker = "from-builtin-custom";
            },
            transform: (): void => {
                seen.push(marker);
            },
        };

        const result: CodeResult = await compile("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [plugin],
        });

        expect(result.code).toBe("const a = 1;");
        expect(seen).toEqual(["from-builtin-custom"]);
    });
});
