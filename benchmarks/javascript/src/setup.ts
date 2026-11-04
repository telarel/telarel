import * as ChildProcess from "node:child_process";
import * as Fsp from "node:fs/promises";
import * as Path from "node:path";
import * as Url from "node:url";

import {
    exists,
    removeStaged,
    stageNative,
    stageWasi,
} from "../../../tests/telarel/src/functions/setup";

type Teardown = () => Promise<void>;

const pkgDir: string = Path.resolve(
    Path.dirname(Url.fileURLToPath(import.meta.url)),
    "..",
    "..",
    "..",
    "packages",
    "telarel",
);

const distDir: string = Path.join(pkgDir, "dist");

const bindingDir: string = Path.join(distDir, "binding");

const assertNativeBindingScriptPath: string = Path.resolve(
    Path.dirname(Url.fileURLToPath(import.meta.url)),
    "..",
    "scripts",
    "assert.ts",
);

// Vitest 5 with Vite 8 crashes while formatting global setup errors here.
// Write the message first so the guard stays visible.
const failSetup = (message: string): never => {
    process.stderr.write(`${message}\n`);
    throw new Error(message);
};

const assertNativeBindingLoads = (): void => {
    const entryPath: string = Path.join(distDir, "index.mjs");

    const entryUrl: string = Url.pathToFileURL(entryPath).href;

    const result: ChildProcess.SpawnSyncReturns<string | undefined> =
        ChildProcess.spawnSync(
            process.execPath,
            [assertNativeBindingScriptPath, entryUrl],
            { encoding: "utf-8" },
        );

    if (result.error === void 0 && result.status === 0) return;

    const stderr: string = (result.stderr ?? "").trim();

    const stdout: string = (result.stdout ?? "").trim();

    const captured: string = stderr.length > 0 ? stderr : stdout;

    const reason: string =
        captured.length > 0
            ? captured
            : (result.error?.message ?? "no error output");

    failSetup(
        [
            `Failed to load a native binding from ${entryPath}.`,
            reason,
            "The benchmark refuses to run on the WASM fallback.",
            "Run `just build-rs-release` to rebuild the native addon.",
        ].join("\n"),
    );
};

const checkNativeBinding = async (): Promise<void> => {
    const hasBindingDir: boolean = await exists(bindingDir);

    const entries: Array<string> = hasBindingDir
        ? await Fsp.readdir(bindingDir)
        : [];

    const hasNativeBinding: boolean = entries.some((entry: string): boolean =>
        entry.endsWith(".node"),
    );

    if (!hasNativeBinding) {
        failSetup(
            `No native binding was staged in ${bindingDir}. Run \`just build-rs-release\` (or \`just bench-js\`, which builds and stages it).`,
        );
    }

    assertNativeBindingLoads();
};

const setup = async (): Promise<Teardown> => {
    await removeStaged();

    const hasDist: boolean = await exists(Path.join(distDir, "index.mjs"));

    if (!hasDist) {
        throw new Error(
            `Missing ${distDir}/index.mjs. Run \`just build-js\` first.`,
        );
    }

    if (process.env.NAPI_RS_FORCE_WASI === "error") {
        await stageWasi();
    } else {
        try {
            await stageNative();
        } catch (error) {
            failSetup(
                `Could not stage the native binding: ${error instanceof Error ? error.message : String(error)}\nThe benchmark requires the release native addon. Run \`just build-rs-release\` (or \`just bench-js\`, which builds it).`,
            );
        }
        await checkNativeBinding();
    }

    return removeStaged;
};

export default setup;
export { setup };
