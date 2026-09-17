import type { CompileOptions, CompileResult, Plugin } from "telarel";

import * as Fs from "node:fs";
import * as Path from "node:path";
import * as Url from "node:url";

import { compile } from "telarel";
import { describe, test } from "vitest";

import { transformPlugin } from "#/plugin";

const benchDir: string = Path.dirname(Url.fileURLToPath(import.meta.url));

const fixturesDir: string = Path.resolve(benchDir, "..", "..", "fixtures");

const fixtures: Array<{ code: string; name: string }> = ["react-page.tsx"].map(
    (name: string): { code: string; name: string } => {
        const code: string = Fs.readFileSync(
            Path.join(fixturesDir, name),
            "utf-8",
        );
        return { code, name };
    },
);

const compileOptions = (code: string): CompileOptions => {
    return {
        cwd: "/repo",
        file: "index.tsx",
        code,
    };
};

const sanity: CompileResult = await compile(compileOptions("const a = 1;"));

if (sanity.code.length === 0) {
    throw new Error("sanity compile produced empty code");
}

const benchCase = (name: string, plugins: Array<Plugin>): void => {
    for (const fixture of fixtures) {
        // oxlint-disable-next-line vitest/expect-expect
        test(`${name} / ${fixture.name}`, async ({ bench }): Promise<void> => {
            await bench(`${name} / ${fixture.name}`, async (): Promise<void> => {
                const result: CompileResult = await compile({
                    ...compileOptions(fixture.code),
                    plugins,
                });
                if (result.code.length === 0) {
                    throw new Error("compile produced empty code");
                }
            }).run();
        });
    }
};

describe("compile", (): void => {
    benchCase("common", []);
    benchCase("plugin", [transformPlugin]);
});
