import type { CodeResult } from "telarel";
import type { TransformOptions } from "telarel/plugins/transform";

import { compile } from "telarel";
import { transform } from "telarel/plugins/transform";
import { describe, expect, it } from "vitest";

describe("transform", (): void => {
    it("compiles TypeScript to JavaScript with the builtin transform", async (): Promise<void> => {
        const result: CodeResult = await compile("const a: number = 1;", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [transform()],
        });

        expect(result.code).not.toContain(": number");
        expect(result.code).toContain("const a");
        expect(result.map?.version).toBe(3);
    });

    it("lowers async await with an es2015 target", async (): Promise<void> => {
        const result: CodeResult = await compile(
            "async function run() { await work(); }",
            {
                cwd: "/repo",
                file: "index.js",
                plugins: [transform({ targets: ["es2015"] })],
            },
        );

        expect(result.code).toContain("_asyncToGenerator");
        expect(result.code).toContain("function run()");
    });

    it("accepts a one-key engine object target", async (): Promise<void> => {
        const result: CodeResult = await compile("const value = maybe?.prop;", {
            cwd: "/repo",
            file: "index.js",
            plugins: [transform({ targets: [{ chrome: 58 }] })],
        });

        expect(result.code).not.toContain("?.");
    });

    it("transforms jsx with the automatic runtime", async (): Promise<void> => {
        const result: CodeResult = await compile(
            "const node = <Button>hi</Button>;",
            {
                cwd: "/repo",
                file: "index.jsx",
                plugins: [transform({ jsx: { runtime: "automatic" } })],
            },
        );

        expect(result.code).not.toContain("<Button>");
        expect(result.code).toContain("jsx");
    });

    it("applies typed oxc escape-hatch options", async (): Promise<void> => {
        const result: CodeResult = await compile(
            "const node = <Button>hi</Button>;",
            {
                cwd: "/repo",
                file: "index.tsx",
                plugins: [
                    transform({
                        jsx: { runtime: "classic" },
                        oxc: { jsx: { pragma: "h" } },
                    }),
                ],
            },
        );

        expect(result.code).toContain("h(");
    });

    it("rewrites relative import extensions", async (): Promise<void> => {
        const result: CodeResult = await compile(
            'import { value } from "./value.ts";\nconsole.log(value);',
            {
                cwd: "/repo",
                file: "index.ts",
                plugins: [
                    transform({
                        typescript: {
                            rewriteRelativeImportExtensions: true,
                        },
                    }),
                ],
            },
        );

        expect(result.code).toContain('"./value.js"');
        expect(result.code).not.toContain('"./value.ts"');
    });

    it("keeps value imports with verbatim module syntax", async (): Promise<void> => {
        const result: CodeResult = await compile(
            [
                'import { used } from "./used";',
                'import { unused } from "./unused";',
                'import { type OnlyType } from "./types";',
                "console.log(used);",
            ].join("\n"),
            {
                cwd: "/repo",
                file: "index.ts",
                plugins: [
                    transform({
                        typescript: { verbatimModuleSyntax: true },
                    }),
                ],
            },
        );

        expect(result.code).toContain('from "./unused"');
        expect(result.code).toContain('import "./types"');
        expect(result.code).not.toContain("OnlyType");
    });

    it("lowers legacy decorators", async (): Promise<void> => {
        const result: CodeResult = await compile(
            "function dec(target: unknown): void {}\n@dec class Example {}",
            {
                cwd: "/repo",
                file: "index.ts",
                plugins: [
                    transform({
                        typescript: { experimentalDecorators: true },
                    }),
                ],
            },
        );

        expect(result.code).not.toContain("@dec");
        expect(result.code).toContain("_decorate");
    });

    it("emits decorator metadata with legacy decorators", async (): Promise<void> => {
        const result: CodeResult = await compile(
            "function dec(target: unknown, key?: unknown): void {}\nclass Example { @dec value!: number; }",
            {
                cwd: "/repo",
                file: "index.ts",
                plugins: [
                    transform({
                        typescript: {
                            emitDecoratorMetadata: true,
                            experimentalDecorators: true,
                        },
                    }),
                ],
            },
        );

        expect(result.code).toContain("design:type");
    });

    it("rejects decorator metadata without legacy decorators", async (): Promise<void> => {
        const promise: Promise<CodeResult> = compile("class Example {}", {
            cwd: "/repo",
            file: "index.ts",
            plugins: [
                transform({
                    typescript: { emitDecoratorMetadata: true },
                }),
            ],
        });

        await expect(promise).rejects.toThrow(/emitDecoratorMetadata/);
        await expect(promise).rejects.toThrow(/experimentalDecorators/);
    });

    it("uses assignment semantics with useDefineForClassFields false", async (): Promise<void> => {
        const result: CodeResult = await compile(
            "class Example { value: number; initialized: number = 1; }",
            {
                cwd: "/repo",
                file: "index.ts",
                plugins: [
                    transform({
                        typescript: { useDefineForClassFields: false },
                    }),
                ],
            },
        );

        expect(result.code).not.toContain("void 0");
        expect(result.code).toContain("this.initialized = 1");
    });

    it("uses define semantics by default", async (): Promise<void> => {
        const result: CodeResult = await compile(
            "class Example { value: number; initialized: number = 1; }",
            {
                cwd: "/repo",
                file: "index.ts",
                plugins: [transform({ targets: ["es2020"] })],
            },
        );

        expect(result.code).toContain("_defineProperty");
        expect(result.code).toContain("void 0");
        expect(result.code).not.toContain("this.initialized = 1");
    });

    it("inlines optimized const enums", async (): Promise<void> => {
        const result: CodeResult = await compile(
            "const enum Direction { Up = 1 }\nconsole.log(Direction.Up);",
            {
                cwd: "/repo",
                file: "index.ts",
                plugins: [
                    transform({
                        typescript: { optimizeConstEnums: true },
                    }),
                ],
            },
        );

        expect(result.code).toContain("console.log(1)");
        expect(result.code).not.toContain("Direction.Up");
    });

    it("uses jsx.pragma for classic import elision", async (): Promise<void> => {
        const result: CodeResult = await compile(
            'import React from "react";\nconst node = <div />;\nconsole.log(node);',
            {
                cwd: "/repo",
                file: "index.tsx",
                plugins: [
                    transform({
                        jsx: { runtime: "classic", pragma: "h" },
                    }),
                ],
            },
        );

        expect(result.code).not.toContain('from "react"');
        expect(result.code).toContain('h("div"');
    });

    it("rejects removed TypeScript option names", async (): Promise<void> => {
        const options: TransformOptions = {
            typescript: {
                onlyRemoveTypeImports: true,
            } as never,
        };

        const build = async (): Promise<CodeResult> =>
            await compile("const value = 1;", {
                cwd: "/repo",
                file: "index.ts",
                plugins: [transform(options)],
            });

        await expect(build()).rejects.toThrow(/typescript options/);
    });

    it("applies oxc helper loader options", async (): Promise<void> => {
        const result: CodeResult = await compile(
            "async function run() { await work(); }",
            {
                cwd: "/repo",
                file: "index.js",
                plugins: [
                    transform({
                        targets: ["es2015"],
                        oxc: { helperLoader: { mode: "external" } },
                    }),
                ],
            },
        );

        expect(result.code).toContain("babelHelpers.");
    });

    it("inlines helper bodies with the inline helper loader mode", async (): Promise<void> => {
        const result: CodeResult = await compile(
            "async function run() { await work(); }",
            {
                cwd: "/repo",
                file: "index.js",
                plugins: [
                    transform({
                        targets: ["es2015"],
                        oxc: { helperLoader: { mode: "inline" } },
                    }),
                ],
            },
        );

        expect(result.code).toContain("function _asyncToGenerator(");
        expect(result.code).toContain("_asyncToGenerator(function* ()");
        expect(result.code).not.toContain("@oxc-project/runtime");
    });

    it("inlines helper bodies for the default helper loader mode", async (): Promise<void> => {
        const result: CodeResult = await compile(
            "async function run() { await work(); }",
            {
                cwd: "/repo",
                file: "index.js",
                plugins: [transform({ targets: ["es2015"] })],
            },
        );

        expect(result.code).toContain("function _asyncToGenerator(");
        expect(result.code).toContain("_asyncToGenerator(function* ()");
        expect(result.code).not.toContain("@oxc-project/runtime");
    });

    it("keeps the runtime helper import with the explicit runtime helper loader mode", async (): Promise<void> => {
        const result: CodeResult = await compile(
            "async function run() { await work(); }",
            {
                cwd: "/repo",
                file: "index.js",
                plugins: [
                    transform({
                        targets: ["es2015"],
                        oxc: { helperLoader: { mode: "runtime" } },
                    }),
                ],
            },
        );

        expect(result.code).toContain(
            "@oxc-project/runtime/helpers/asyncToGenerator",
        );
        expect(result.code).not.toContain("function _asyncToGenerator(");
    });

    it("replaces a defined identifier with its value", async (): Promise<void> => {
        const result: CodeResult = await compile("const flag = __DEV__;", {
            cwd: "/repo",
            file: "index.js",
            plugins: [transform({ define: { __DEV__: "false" } })],
        });

        expect(result.code).toContain("false");
        expect(result.code).not.toContain("__DEV__");
    });

    it("replaces a defined member chain with a folded literal", async (): Promise<void> => {
        const result: CodeResult = await compile(
            "const mode = process.env.NODE_ENV;",
            {
                cwd: "/repo",
                file: "index.js",
                plugins: [
                    transform({
                        define: { "process.env.NODE_ENV": '"production"' },
                    }),
                ],
            },
        );

        expect(result.code).toContain('"production"');
        expect(result.code).not.toContain("process.env");
    });

    it("combines define with target lowering", async (): Promise<void> => {
        const result: CodeResult = await compile(
            "async function run() { await work(); } const flag = __DEV__;",
            {
                cwd: "/repo",
                file: "index.js",
                plugins: [
                    transform({
                        targets: ["es2015"],
                        define: { __DEV__: "true" },
                    }),
                ],
            },
        );

        expect(result.code).toContain("_asyncToGenerator");
        expect(result.code).toContain("true");
        expect(result.code).not.toContain("__DEV__");
    });

    it("injects an import for a used shorthand identifier", async (): Promise<void> => {
        const result: CodeResult = await compile("const node = $('#root');", {
            cwd: "/repo",
            file: "index.js",
            plugins: [transform({ inject: { $: "jquery" } })],
        });

        expect(result.code).toContain("jquery");
        expect(result.code).toContain("import");
    });

    it("injects no import for an unused shorthand identifier", async (): Promise<void> => {
        const result: CodeResult = await compile(
            "const node = query('#root');",
            {
                cwd: "/repo",
                file: "index.js",
                plugins: [transform({ inject: { $: "jquery" } })],
            },
        );

        expect(result.code).not.toContain("jquery");
        expect(result.code).not.toContain("import");
    });

    it("injects a namespace import in object form", async (): Promise<void> => {
        const result: CodeResult = await compile(
            "const node = React.createElement('div');",
            {
                cwd: "/repo",
                file: "index.js",
                plugins: [
                    transform({
                        inject: {
                            React: {
                                source: "react",
                                mode: "namespace",
                                local: "React",
                            },
                        },
                    }),
                ],
            },
        );

        expect(result.code).toContain("import * as React");
    });

    it("injects a named import from the pair form", async (): Promise<void> => {
        const result: CodeResult = await compile(
            "const merged = assign({}, source);",
            {
                cwd: "/repo",
                file: "index.js",
                plugins: [
                    transform({
                        inject: { assign: ["es-object-assign", "assign"] },
                    }),
                ],
            },
        );

        expect(result.code).toContain("es-object-assign");
        expect(result.code).toContain("import");
    });

    it("rejects an invalid define key", async (): Promise<void> => {
        const options: TransformOptions = { define: { "1bad": "1" } };

        const build = async (): Promise<CodeResult> =>
            await compile("const a = 1;", {
                cwd: "/repo",
                file: "index.js",
                plugins: [transform(options)],
            });

        const promise: Promise<CodeResult> = build();

        await expect(promise).rejects.toThrow(/define/);
        await expect(promise).rejects.toThrow("1bad");
    });

    it("folds a scalar define value into the output", async (): Promise<void> => {
        const result: CodeResult = await compile("const enabled = ENABLED;", {
            cwd: "/repo",
            file: "index.js",
            plugins: [transform({ define: { ENABLED: true } })],
        });

        expect(result.code).toContain("true");
        expect(result.code).not.toContain("ENABLED");
    });
});
