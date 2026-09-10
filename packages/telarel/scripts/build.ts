import type { SpawnSyncReturns } from "node:child_process";

import type { ConsolaInstance } from "consola";

import { spawnSync } from "node:child_process";
import * as Fsp from "node:fs/promises";
import * as Path from "node:path";
import { fileURLToPath } from "node:url";

import { NapiCli } from "@napi-rs/cli";
import { createConsola } from "consola";

const __dirname: string = Path.dirname(fileURLToPath(import.meta.url));
const pkgDir: string = Path.resolve(__dirname, "..");

const consola: ConsolaInstance = createConsola({
    formatOptions: {
        date: false,
    },
});

type PackageJson = {
    napi: {
        targets: ReadonlyArray<string>;
    };
};

// oxlint-disable-next-line typescript/no-unsafe-assignment
const pkg: PackageJson = JSON.parse(
    await Fsp.readFile(Path.resolve(pkgDir, "package.json"), "utf-8"),
);

const platforms: ReadonlyArray<string> = pkg.napi.targets;

await Fsp.mkdir(Path.resolve(pkgDir, Path.join("src", "binding")), {
    recursive: true,
});

const napi: NapiCli = new NapiCli();

const rustcVersion: SpawnSyncReturns<string> = spawnSync("rustc", ["-vV"], {
    encoding: "utf-8",
});

const lines: string[] = rustcVersion.stdout.split("\n");

let hostTriple: string = "";

for (const line of lines) {
    if (line.startsWith("host: ")) {
        hostTriple = line.slice(6);
        break;
    }
}

if (rustcVersion.status !== 0 || hostTriple === "") {
    throw new Error("Failed to detect host target triple via `rustc -vV`.");
}

if (!platforms.includes(hostTriple)) {
    throw new Error(
        `Host triple "${hostTriple}" is not present in napi targets [${platforms.join(", ")}].`,
    );
}

const hasWasm: boolean = platforms.includes("wasm32-wasip1-threads");

const wasmTarget: string = "wasm32-wasip1-threads";

const orderedTargets: ReadonlyArray<string> = [
    hostTriple,
    ...(hasWasm ? [wasmTarget] : []),
];

for (const platform of orderedTargets) {
    consola.info(`Building Rust binding for ${platform}...`);

    const isWasmTarget: boolean = platform === "wasm32-wasip1-threads";

    const { task: buildTask } = await napi.build({
        target: platform,
        cwd: pkgDir,
        manifestPath: Path.join("..", "..", "Cargo.toml"),
        packageJsonPath: Path.join("package.json"),
        outputDir: Path.join("artifacts"),
        platform: true,
        jsPackageName: "@telarel/binding",
        jsBinding: Path.join("..", "src", "binding", "index.js"),
        dts: Path.join("..", "src", "binding", "index.d.ts"),
        esm: true,
        package: "telarel_binding",
        profile: isWasmTarget ? "dev-wasm" : void 0,
    });

    await buildTask;
}

consola.info("Moving artifacts into npm packages...");

await napi.artifacts({
    cwd: pkgDir,
    packageJsonPath: Path.join("package.json"),
    outputDir: Path.join("artifacts"),
    npmDir: Path.join("npm"),
    buildOutputDir: Path.join("artifacts"),
});

consola.info("Cleaning up stray build artifacts from package root...");

const isStrayArtifact = (name: string): boolean =>
    name.endsWith(".node") ||
    name.endsWith(".wasm") ||
    name.endsWith(".wasi.cjs") ||
    name.endsWith(".wasi-browser.js") ||
    name.startsWith("wasi-worker") ||
    name === "browser.js";

for (const file of await Fsp.readdir(pkgDir)) {
    if (isStrayArtifact(file)) {
        await Fsp.rm(Path.resolve(pkgDir, file));
    }
}

consola.success("Build complete");
