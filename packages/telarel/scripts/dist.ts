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

const nativePlatforms: ReadonlyArray<string> = platforms.filter(
    (p: string): boolean => p !== "wasm32-wasip1-threads",
);

const wasmPlatforms: ReadonlyArray<string> = platforms.filter(
    (p: string): boolean => p === "wasm32-wasip1-threads",
);

const orderedPlatforms: ReadonlyArray<string> = [
    ...nativePlatforms,
    ...wasmPlatforms,
];

for (const platform of orderedPlatforms) {
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
        profile: isWasmTarget ? "release-wasm" : "release",
        crossCompile: true,
    });

    await buildTask;
}

const wasmArtifactPath: string = Path.join(
    pkgDir,
    "artifacts",
    "telarel.wasm32-wasi.wasm",
);

const formatBytes = (bytes: number): string => {
    if (bytes < 1024) return `${bytes}B`;

    const units: ReadonlyArray<string> = ["KB", "MB", "GB"];
    let value: number = bytes / 1024;
    let unitIndex: number = 0;

    while (value >= 1024 && unitIndex < units.length - 1) {
        value /= 1024;
        unitIndex += 1;
    }

    return `${value.toFixed(2)}${units[unitIndex]}`;
};

const optimizeWasm = async (): Promise<void> => {
    const exists: boolean = await Fsp.access(wasmArtifactPath)
        .then((): boolean => true)
        .catch((): boolean => false);

    if (!exists) return void 0;

    consola.info("Optimizing WASM with wasm-opt...");

    const sizeBefore: number = (await Fsp.stat(wasmArtifactPath)).size;

    const result: SpawnSyncReturns<Buffer> = spawnSync(
        "wasm-opt",
        [
            "-Oz",
            "--enable-threads",
            "--enable-bulk-memory",
            "--enable-nontrapping-float-to-int",
            wasmArtifactPath,
            "-o",
            wasmArtifactPath,
        ],
        {
            stdio: "inherit",
        },
    );

    if (result.error) {
        consola.warn(
            "wasm-opt not found on PATH; skipping WASM size optimization.",
        );
    } else if (result.status !== 0) {
        consola.warn(
            `wasm-opt exited with status ${result.status}; keeping unoptimized WASM.`,
        );
    } else {
        const sizeAfter: number = (await Fsp.stat(wasmArtifactPath)).size;

        const delta: number = sizeAfter - sizeBefore;
        const sign: string = delta <= 0 ? "" : "+";
        const pct: string = ((delta / sizeBefore) * 100).toFixed(1);

        consola.success(
            `Optimized WASM: ${formatBytes(sizeBefore)} -> ${formatBytes(sizeAfter)} (${sign}${pct}%)`,
        );
    }
};

await optimizeWasm();

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
