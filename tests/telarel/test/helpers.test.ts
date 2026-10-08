import type { CompileResult } from "telarel";
import type { TransformOptions } from "telarel/plugins/transform";

import * as Fsp from "node:fs/promises";
import * as Path from "node:path";
import * as Url from "node:url";

import { compile } from "telarel";
import { transform } from "telarel/plugins/transform";
import { describe, expect, it } from "vitest";

const MODULE_NAME = "@oxc-project/runtime" as const;

const testDir: string = Path.dirname(Url.fileURLToPath(import.meta.url));

const repoDir: string = Path.resolve(testDir, "..", "..", "..");

const sourcesPath: string = Path.resolve(
    repoDir,
    "crates",
    "plugin_transform",
    "src",
    "helpers",
    "sources.rs",
);

/**
 * The four helpers the JS binding cannot reach: legacy decorators and the
 * tagged-template-escape plugin are not exposed through the `oxc` passthrough.
 * They are covered by the Rust trigger table instead.
 */
const BINDING_UNREACHABLE: ReadonlySet<string> = new Set([
    "decorate",
    "decorateParam",
    "decorateMetadata",
    "taggedTemplateLiteral",
]);

const readHelperNames = async (): Promise<Array<string>> => {
    const sources: string = await Fsp.readFile(sourcesPath, "utf-8");

    const names: Array<string> = [...sources.matchAll(/name: "([^"]+)"/gu)].map(
        (match: RegExpMatchArray): string => match[1] ?? "",
    );

    return names.filter((name: string): boolean => name.length > 0);
};

/**
 * One snippet that makes the transform plugin emit a specific runtime helper.
 *
 * `dependencyMarker` is set for the dependency-only helpers oxc never loads
 * directly (`OverloadYield`, `get`, `typeof`, ...): they arrive transitively
 * from a dependent, so they are proven by a rename-stable marker in the inlined
 * body rather than a runtime import specifier.
 */
type Trigger = {
    helper: string;
    file: string;
    code: string;
    options: TransformOptions;
    dependencyMarker?: string;
};

const es2015: Array<"es2015"> = ["es2015"];

const es2017: Array<"es2017"> = ["es2017"];

type InlineModeOptions = {
    options: TransformOptions;
    mode: "inline" | "runtime";
};

const inlineMode = ({
    options,
    mode,
}: InlineModeOptions): TransformOptions => ({
    ...options,
    oxc: {
        ...options.oxc,
        helperLoader: { mode },
    },
});

const TRIGGERS: ReadonlyArray<Trigger> = [
    {
        helper: "awaitAsyncGenerator",
        file: "index.mjs",
        code: "async function* g() { await x; }",
        options: { targets: es2017 },
    },
    {
        helper: "asyncGeneratorDelegate",
        file: "index.mjs",
        code: "async function* g() { yield* x; }",
        options: { targets: es2017 },
    },
    {
        helper: "asyncIterator",
        file: "index.mjs",
        code: "async function f() { for await (const x of y) {} }",
        options: { targets: es2015 },
    },
    {
        helper: "asyncToGenerator",
        file: "index.mjs",
        code: "async function f() { await x; }",
        options: { targets: es2015 },
    },
    {
        helper: "objectSpread2",
        file: "index.mjs",
        code: "const o = { ...a, ...b };",
        options: { targets: es2015 },
    },
    {
        helper: "wrapAsyncGenerator",
        file: "index.mjs",
        code: "async function* g() { yield 1; }",
        options: { targets: es2017 },
    },
    {
        helper: "extends",
        file: "index.mjs",
        code: "const { ...r } = obj;",
        options: { targets: es2015 },
    },
    {
        helper: "objectDestructuringEmpty",
        file: "index.mjs",
        code: "const { ...r } = obj;",
        options: { targets: es2015 },
    },
    {
        helper: "objectWithoutProperties",
        file: "index.mjs",
        code: "const { a, ...r } = obj;",
        options: { targets: es2015 },
    },
    {
        helper: "toPropertyKey",
        file: "index.mjs",
        code: "const { [k]: v, ...r } = obj;",
        options: { targets: es2015 },
    },
    {
        helper: "defineProperty",
        file: "index.mjs",
        code: "class C { x = 1; }",
        options: { targets: es2015 },
    },
    {
        helper: "classPrivateFieldInitSpec",
        file: "index.mjs",
        code: "class C { #x = 1; m() { return this.#x; } }",
        options: { targets: es2015 },
    },
    {
        helper: "classPrivateMethodInitSpec",
        file: "index.mjs",
        code: "class C { #m() {} c() { this.#m(); } }",
        options: { targets: es2015 },
    },
    {
        helper: "classPrivateFieldGet2",
        file: "index.mjs",
        code: "class C { #x = 1; m() { return this.#x; } }",
        options: { targets: es2015 },
    },
    {
        helper: "classPrivateFieldSet2",
        file: "index.mjs",
        code: "class C { #x = 1; m() { this.#x = 2; } }",
        options: { targets: es2015 },
    },
    {
        helper: "assertClassBrand",
        file: "index.mjs",
        code: "class C { #m() {} c() { this.#m(); } }",
        options: { targets: es2015 },
    },
    {
        helper: "toSetter",
        file: "index.mjs",
        code: "class C { set #x(v) {} m() { this.#x = 1; } }",
        options: { targets: es2015 },
    },
    {
        helper: "classPrivateFieldLooseKey",
        file: "index.mjs",
        code: "class C { #x = 1; m() { return this.#x; } }",
        options: {
            targets: es2015,
            oxc: { assumptions: { privateFieldsAsProperties: true } },
        },
    },
    {
        helper: "classPrivateFieldLooseBase",
        file: "index.mjs",
        code: "class C { #x = 1; m() { return this.#x; } }",
        options: {
            targets: es2015,
            oxc: { assumptions: { privateFieldsAsProperties: true } },
        },
    },
    {
        helper: "superPropGet",
        file: "index.mjs",
        code: "class C extends B { static { const z = super.x; } }",
        options: { targets: es2015 },
    },
    {
        helper: "superPropSet",
        file: "index.mjs",
        code: "class C extends B { static { super.x = 1; } }",
        options: { targets: es2015 },
    },
    {
        helper: "readOnlyError",
        file: "index.mjs",
        code: "class C { get #x() { return 1; } m() { this.#x = 2; } }",
        options: { targets: es2015 },
    },
    {
        helper: "writeOnlyError",
        file: "index.mjs",
        code: "class C { set #x(v) {} m() { return this.#x; } }",
        options: { targets: es2015 },
    },
    {
        helper: "checkInRHS",
        file: "index.mjs",
        code: "class C { #x; has(o) { return #x in o; } }",
        options: { targets: es2015 },
    },
    {
        helper: "usingCtx",
        file: "index.mjs",
        code: "using x = y;",
        options: { targets: es2015 },
    },
    {
        helper: "OverloadYield",
        file: "index.mjs",
        code: "async function* g() { yield 1; }",
        options: { targets: es2017 },
        dependencyMarker: "this.v = e, this.k = d",
    },
    {
        helper: "checkPrivateRedeclaration",
        file: "index.mjs",
        code: "class C { #x = 1; m() { return this.#x; } }",
        options: { targets: es2015 },
        dependencyMarker:
            "Cannot initialize the same private elements twice on an object",
    },
    {
        helper: "get",
        file: "index.mjs",
        code: "class C extends B { static { const z = super.x; } }",
        options: { targets: es2015 },
        dependencyMarker: "Reflect.get.bind",
    },
    {
        helper: "getPrototypeOf",
        file: "index.mjs",
        code: "class C extends B { static { const z = super.x; } }",
        options: { targets: es2015 },
        dependencyMarker:
            "Object.setPrototypeOf ? Object.getPrototypeOf.bind()",
    },
    {
        helper: "objectWithoutPropertiesLoose",
        file: "index.mjs",
        code: "const { a, ...r } = obj;",
        options: { targets: es2015 },
        dependencyMarker: ".includes(n)) continue",
    },
    {
        helper: "set",
        file: "index.mjs",
        code: "class C extends B { static { super.x = 1; } }",
        options: { targets: es2015 },
        dependencyMarker: "failed to set property",
    },
    {
        helper: "superPropBase",
        file: "index.mjs",
        code: "class C extends B { static { const z = super.x; } }",
        options: { targets: es2015 },
        dependencyMarker: "!{}.hasOwnProperty.call(t, o) && null !==",
    },
    {
        helper: "toPrimitive",
        file: "index.mjs",
        code: "const o = { ...a, ...b };",
        options: { targets: es2015 },
        dependencyMarker: "@@toPrimitive must return a primitive value.",
    },
    {
        helper: "typeof",
        file: "index.mjs",
        code: "const o = { ...a, ...b };",
        options: { targets: es2015 },
        dependencyMarker: "@babel/helpers - typeof",
    },
];

const specifierFor = (helper: string): string =>
    `${MODULE_NAME}/helpers/${helper}`;

/**
 * The local binding oxc gave the default import/require of `specifier`, or
 * `undefined` when the runtime pass did not emit one.
 */
type RuntimeImportLocalOptions = {
    code: string;
    specifier: string;
};

const runtimeImportLocal = ({
    code,
    specifier,
}: RuntimeImportLocalOptions): string | undefined => {
    for (const raw of code.split("\n")) {
        const line: string = raw.trim();

        if (line.startsWith("import ") && line.includes(`"${specifier}"`)) {
            const local: string = line.split(" from ")[0] ?? "";

            return local.replace(/^import\s+/u, "").trim();
        }

        if (
            line.startsWith("var ") &&
            line.includes(`require("${specifier}")`)
        ) {
            const local: string = line.split(" = ")[0] ?? "";

            return local.replace(/^var\s+/u, "").trim();
        }
    }

    return void 0;
};

type CompileInModeOptions = {
    trigger: Trigger;
    mode: "inline" | "runtime";
};

const compileInMode = async ({
    trigger,
    mode,
}: CompileInModeOptions): Promise<CompileResult> =>
    await compile({
        cwd: "/repo",
        file: trigger.file,
        code: trigger.code,
        plugins: [transform(inlineMode({ options: trigger.options, mode }))],
    });

const directTriggers: ReadonlyArray<Trigger> = TRIGGERS.filter(
    (trigger: Trigger): boolean => trigger.dependencyMarker === void 0,
);

const dependencyTriggers: ReadonlyArray<[Trigger, string]> = TRIGGERS.flatMap(
    (trigger: Trigger): Array<[Trigger, string]> =>
        trigger.dependencyMarker === void 0
            ? []
            : [[trigger, trigger.dependencyMarker]],
);

describe("transform helpers", (): void => {
    for (const trigger of directTriggers) {
        it(`emits and inlines ${trigger.helper}`, async (): Promise<void> => {
            const runtime: CompileResult = await compileInMode({
                trigger,
                mode: "runtime",
            });
            const inlined: CompileResult = await compileInMode({
                trigger,
                mode: "inline",
            });

            // The snippet must make the plugin emit the runtime import for
            // this helper...
            expect(runtime.code).toContain(specifierFor(trigger.helper));

            // ...and the inline pass must bind the helper's body under the
            // very local name oxc chose for that import, proving the emitted
            // import was replaced by the inlined function.
            const local: string | undefined = runtimeImportLocal({
                code: runtime.code,
                specifier: specifierFor(trigger.helper),
            });

            expect(local).toBeDefined();
            expect(inlined.code).toContain(`function ${local}(`);

            // ...and the inline pass must consume it.
            expect(inlined.code).not.toContain(MODULE_NAME);
        });
    }

    for (const [trigger, marker] of dependencyTriggers) {
        it(`inlines dependency ${trigger.helper}`, async (): Promise<void> => {
            const runtime: CompileResult = await compileInMode({
                trigger,
                mode: "runtime",
            });
            const inlined: CompileResult = await compileInMode({
                trigger,
                mode: "inline",
            });

            // A dependency-only helper is never loaded directly by oxc; it
            // arrives transitively, so it is proven by its inlined body.
            expect(runtime.code).not.toContain(specifierFor(trigger.helper));
            expect(inlined.code).toContain(marker);
            expect(inlined.code).not.toContain(MODULE_NAME);
        });
    }

    it("covers every helper reachable from the JS binding", async (): Promise<void> => {
        const names: Array<string> = await readHelperNames();

        expect(names.length).toBeGreaterThan(0);

        const expected: Array<string> = names.filter(
            (name: string): boolean => !BINDING_UNREACHABLE.has(name),
        );
        const covered: Array<string> = TRIGGERS.map(
            (trigger: Trigger): string => trigger.helper,
        );

        for (const name of expected) {
            expect(covered).toContain(name);
        }

        expect(covered).toHaveLength(expected.length);
    });
});
