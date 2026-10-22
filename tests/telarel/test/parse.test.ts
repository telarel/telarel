import type {
    AstOptions,
    AstResult,
    CommonPluginContext,
    OptionsArgs,
    ParseArgs,
    ParserOptions,
    Plugin,
    PluginContext,
} from "telarel";

import * as Fsp from "node:fs/promises";
import * as Path from "node:path";

import { CompileError, generate, parse } from "telarel";
import { afterEach, describe, expect, it } from "vitest";

import {
    appendInlineMap,
    buildMap,
    makeTempDir,
    removeTempDir,
} from "#/functions/sourcemap";

const tempDirs: Array<string> = [];

// NAPI-RS preopens the drive root as a guest path like `D:\`, and wasi-libc
// only recognizes `/` separators. Filesystem-backed input maps therefore
// cannot resolve Windows absolute paths under WASI.
const isWasiWindows: boolean =
    process.env.NAPI_RS_FORCE_WASI === "error" && process.platform === "win32";

const tempDir = async (): Promise<string> => {
    const directory: string = await makeTempDir("telarel-parse-");

    tempDirs.push(directory);

    return directory;
};

afterEach(async (): Promise<void> => {
    await Promise.all(tempDirs.splice(0).map(removeTempDir));
});

const upstreamMap = (): ReturnType<typeof buildMap> =>
    buildMap({
        file: "upstream.ts",
        mappings: "AASA",
        names: [],
        sources: ["upstream.ts"],
        sourcesContent: ["const a = 1;"],
    });

describe("parse", (): void => {
    it("returns the source and program in an AST result", async (): Promise<void> => {
        const result: AstResult = await parse("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
        });

        expect(result.ast.source).toBe("const a = 1;");
        expect(result.ast.program.type).toBe("Program");
        // Input maps are off by default: no map is attached.
        expect(result.ast.map).toBeNull();
    });

    it("round-trips the parsed program through generate", async (): Promise<void> => {
        const parsed: AstResult = await parse("const a = 1;", {
            file: "index.ts",
        });

        const fromAst = await generate(parsed.ast, { file: "index.ts" });
        const fromProgram = await generate(
            parsed.ast.program as unknown as AstOptions["program"],
            parsed.ast.source,
            {
                file: "index.ts",
            },
        );

        expect(fromAst.code).toBe("const a = 1;");
        expect(fromProgram.code).toBe("const a = 1;");
    });

    it("rejects invalid code with a file-context parse error", async (): Promise<void> => {
        await expect(
            parse("const = ;", { cwd: "/repo", file: "index.ts" }),
        ).rejects.toSatisfy(
            (error: unknown): boolean =>
                error instanceof CompileError &&
                error.render().includes("index.ts"),
        );

        await expect(
            parse("const = ;", { cwd: "/repo", file: "index.ts" }),
        ).rejects.toThrow(/expected|Unexpected token/i);
    });

    it("fires options, start, parse, and end exactly once", async (): Promise<void> => {
        const seen: Array<string> = [];

        await parse("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "observer",
                    options: (): void => {
                        seen.push("options");
                    },
                    start: (): void => {
                        seen.push("start");
                    },
                    parse: (): void => {
                        seen.push("parse");
                    },
                    prepare: (): void => {
                        seen.push("prepare");
                    },
                    transform: (): void => {
                        seen.push("transform");
                    },
                    finalize: (): void => {
                        seen.push("finalize");
                    },
                    generate: (): void => {
                        seen.push("generate");
                    },
                    end: (): void => {
                        seen.push("end");
                    },
                },
            ],
        });

        expect(seen).toEqual(["options", "start", "parse", "end"]);
    });

    it("exposes the code and resolved parser options to the parse hook", async (): Promise<void> => {
        const seen: Array<{ keys: Array<string>; options: ParserOptions }> = [];

        await parse("const a = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                {
                    name: "observe-parse",
                    parse: (_ctx: PluginContext, args: ParseArgs): void => {
                        seen.push({
                            keys: Object.keys(args),
                            options: args.options,
                        });
                    },
                },
            ],
        });

        expect(seen).toHaveLength(1);
        expect(seen[0]?.keys).toEqual(["code", "options"]);
        expect(seen[0]?.options.file).toBe("index.ts");
        expect(seen[0]?.options.cwd).toBe("/repo");
        expect(seen[0]?.options.language).toBe("ts");
        expect(seen[0]?.options.sourceType).toBe("unambiguous");
    });

    it("attaches a value input map to the AST", async (): Promise<void> => {
        const result: AstResult = await parse("const a = 1;", {
            file: "index.ts",
            inputSourceMap: upstreamMap(),
        });

        expect(result.ast.map?.sources).toEqual(["upstream.ts"]);
        expect(result.ast.map?.mappings).toBe("AASA");
    });

    it("attaches a value input map from options and ignores its path", async (): Promise<void> => {
        const result: AstResult = await parse("const a = 1;", {
            file: "index.ts",
            inputSourceMap: {
                mode: upstreamMap(),
                path: "ignored.map",
            },
        });

        expect(result.ast.map?.sources).toEqual(["upstream.ts"]);
        expect(result.ast.map?.mappings).toBe("AASA");
    });

    it("attaches an inline input map from a code comment", async (): Promise<void> => {
        const code: string = appendInlineMap("const a = 1;", upstreamMap());

        const result: AstResult = await parse(code, {
            file: "index.ts",
            inputSourceMap: "inline",
        });

        expect(result.ast.map?.sources).toEqual(["upstream.ts"]);
    });

    it("ignores an output map set by an options hook", async (): Promise<void> => {
        const code: string = appendInlineMap("const a = 1;", upstreamMap());

        const result: AstResult = await parse(code, {
            file: "index.ts",
            inputSourceMap: "inline",
            plugins: [
                {
                    name: "output-for-every-stage",
                    options: (
                        _ctx: CommonPluginContext,
                        args: OptionsArgs,
                    ): OptionsArgs => ({
                        options: {
                            ...args.options,
                            sourceMap: {
                                ...args.options.sourceMap,
                                output: "inline",
                            },
                        },
                    }),
                },
            ],
        });

        expect(result.ast.map?.sources).toEqual(["upstream.ts"]);
    });

    it("ignores a top-level output set by an options hook", async (): Promise<void> => {
        const code: string = appendInlineMap("const a = 1;", upstreamMap());

        const result: AstResult = await parse(code, {
            file: "index.ts",
            inputSourceMap: "inline",
            plugins: [
                {
                    name: "set-top-level-output",
                    options: (
                        _ctx: CommonPluginContext,
                        args: OptionsArgs,
                    ): OptionsArgs => ({
                        options: {
                            ...args.options,
                            output: { file: "dist/index.js" },
                        },
                    }),
                },
            ],
        });

        expect(result.ast.source).toBe(code);
        expect(result.ast.map?.sources).toEqual(["upstream.ts"]);
        expect(result.ast.map?.mappings).toBe("AASA");
    });

    it.skipIf(isWasiWindows)(
        "attaches an external input map read next to the source",
        async (): Promise<void> => {
            const directory: string = await tempDir();

            await Fsp.writeFile(
                Path.join(directory, "index.ts.map"),
                JSON.stringify(upstreamMap()),
                "utf-8",
            );

            const result: AstResult = await parse("const a = 1;", {
                cwd: directory,
                file: "index.ts",
                inputSourceMap: "external",
            });

            expect(result.ast.map?.sources).toEqual(["upstream.ts"]);
        },
    );

    it.skipIf(isWasiWindows)(
        "attaches an external input map read from a custom path",
        async (): Promise<void> => {
            const directory: string = await tempDir();

            await Fsp.writeFile(
                Path.join(directory, "custom.map"),
                JSON.stringify(upstreamMap()),
                "utf-8",
            );

            const result: AstResult = await parse("const a = 1;", {
                cwd: directory,
                file: "index.ts",
                inputSourceMap: {
                    mode: "external",
                    path: "custom.map",
                },
            });

            expect(result.ast.map?.sources).toEqual(["upstream.ts"]);
        },
    );

    it.skipIf(isWasiWindows)(
        "resolves a relative input map path against the source file's directory",
        async (): Promise<void> => {
            const directory: string = await tempDir();

            await Fsp.mkdir(Path.join(directory, "src"), {
                recursive: true,
            });

            await Fsp.writeFile(
                Path.join(directory, "src", "custom.map"),
                JSON.stringify(upstreamMap()),
                "utf-8",
            );

            const result: AstResult = await parse("const a = 1;", {
                cwd: directory,
                file: "src/index.ts",
                inputSourceMap: {
                    mode: "external",
                    path: "custom.map",
                },
            });

            expect(result.ast.map?.sources).toEqual(["upstream.ts"]);
        },
    );

    it("rejects a path string as inputSourceMap", async (): Promise<void> => {
        await expect(
            parse("const a = 1;", {
                file: "index.ts",
                inputSourceMap: "custom.map" as never,
            }),
        ).rejects.toThrow("invalid inputSourceMap");
    });

    it("prefers the inline map in auto mode", async (): Promise<void> => {
        const directory: string = await tempDir();

        await Fsp.writeFile(
            Path.join(directory, "index.ts.map"),
            JSON.stringify(upstreamMap()),
            "utf-8",
        );

        const inlineMap = buildMap({
            file: "inline.ts",
            mappings: "AAAA",
            sources: ["inline.ts"],
        });

        const fromInline: AstResult = await parse(
            appendInlineMap("const a = 1;", inlineMap),
            {
                cwd: directory,
                file: "index.ts",
                inputSourceMap: "auto",
            },
        );

        expect(fromInline.ast.map?.sources).toEqual(["inline.ts"]);
    });

    it.skipIf(isWasiWindows)(
        "falls back to the external map in auto mode",
        async (): Promise<void> => {
            const directory: string = await tempDir();

            await Fsp.writeFile(
                Path.join(directory, "index.ts.map"),
                JSON.stringify(upstreamMap()),
                "utf-8",
            );

            const fromExternal: AstResult = await parse("const a = 1;", {
                cwd: directory,
                file: "index.ts",
                inputSourceMap: "auto",
            });

            expect(fromExternal.ast.map?.sources).toEqual(["upstream.ts"]);
        },
    );

    it.skipIf(isWasiWindows)(
        "falls back to a custom external map in auto mode",
        async (): Promise<void> => {
            const directory: string = await tempDir();

            await Fsp.writeFile(
                Path.join(directory, "custom.map"),
                JSON.stringify(upstreamMap()),
                "utf-8",
            );

            const result: AstResult = await parse("const a = 1;", {
                cwd: directory,
                file: "index.ts",
                inputSourceMap: {
                    mode: "auto",
                    path: "custom.map",
                },
            });

            expect(result.ast.map?.sources).toEqual(["upstream.ts"]);
        },
    );

    it("attaches no map in auto mode when none is available", async (): Promise<void> => {
        const directory: string = await tempDir();

        const result: AstResult = await parse("const a = 1;", {
            cwd: directory,
            file: "index.ts",
            inputSourceMap: "auto",
        });

        expect(result.ast.map).toBeNull();
    });

    it("rejects a plugin with a missing name", async (): Promise<void> => {
        const nameless: unknown = { prepare: (): void => {} };

        await expect(
            parse("const a = 1;", {
                file: "index.ts",
                plugins: [nameless as Plugin],
            }),
        ).rejects.toThrow("plugin.name must be a non-empty string");
    });

    it("rejects a bare function plugin entry", async (): Promise<void> => {
        const factory: Plugin = (() => void 0) as unknown as Plugin;

        await expect(
            parse("const a = 1;", {
                file: "index.ts",
                plugins: [factory],
            }),
        ).rejects.toThrow("pass a plugin object, not a factory");
    });
});
