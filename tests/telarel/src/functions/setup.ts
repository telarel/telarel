import * as Fs from "node:fs";
import * as Fsp from "node:fs/promises";
import * as Path from "node:path";
import * as Url from "node:url";

type StageOptions = {
    target: string;
    source: string;
};

type Teardown = () => Promise<void>;

type PackageManifest = {
    directory: string;
    main: string;
    os?: ReadonlyArray<string>;
    cpu?: ReadonlyArray<string>;
};

const testDir: string = Path.dirname(Url.fileURLToPath(import.meta.url));
const repoDir: string = Path.resolve(testDir, "..", "..", "..", "..");
const pkgDir: string = Path.resolve(repoDir, "packages", "telarel");
const distDir: string = Path.join(pkgDir, "dist");
const bindingDir: string = Path.join(distDir, "binding");
const npmDir: string = Path.join(pkgDir, "npm");

const staged: Array<string> = [];
const deferred: Array<string> = [];

const LOCK_ERROR_CODES: ReadonlyArray<string> = ["EPERM", "EBUSY"];

const isLockError = (error: unknown): boolean =>
    error instanceof Error &&
    LOCK_ERROR_CODES.includes((error as NodeJS.ErrnoException).code ?? "");

const exists = async (path: string): Promise<boolean> => {
    try {
        await Fsp.stat(path);
        return true;
    } catch {
        return false;
    }
};

const clear = async (target: string): Promise<void> => {
    await Fsp.rm(target, { force: true, recursive: true });
};

const stage = async (options: StageOptions): Promise<void> => {
    await clear(options.target);
    await Fsp.copyFile(options.source, options.target);
    staged.push(options.target);
};

const matchesConstraint = (
    constraint: ReadonlyArray<string> | undefined,
    value: string,
): boolean => constraint === void 0 || constraint.includes(value);

/**
 * Selects the npm packages whose `os`/`cpu` metadata matches the running
 * platform. An absent constraint is a wildcard, while a `libc` mismatch is
 * ignored on purpose: every matching variant is staged and the generated loader
 * (`src/binding/index.js`) picks the right one at runtime.
 */
const selectPlatformPackages = (
    manifests: ReadonlyArray<PackageManifest>,
    platform: string,
    arch: string,
): Array<PackageManifest> =>
    manifests.filter(
        (manifest: PackageManifest): boolean =>
            matchesConstraint(manifest.os, platform) &&
            matchesConstraint(manifest.cpu, arch),
    );

const readManifest = async (directory: string): Promise<PackageManifest> => {
    const raw: string = await Fsp.readFile(
        Path.join(npmDir, directory, "package.json"),
        "utf-8",
    );

    type ParsedManifest = {
        main?: string;
        os?: Array<string>;
        cpu?: Array<string>;
    };

    // oxlint-disable-next-line typescript/no-unsafe-assignment
    const parsed: ParsedManifest = JSON.parse(raw);

    if (parsed.main === void 0) {
        throw new Error(`Missing "main" in ${directory}/package.json.`);
    }

    return {
        directory,
        main: parsed.main,
        os: parsed.os,
        cpu: parsed.cpu,
    };
};

const stageNative = async (): Promise<void> => {
    const directories: Array<string> = await Fsp.readdir(npmDir);

    const manifests: Array<PackageManifest> = await Promise.all(
        directories.map(
            async (directory: string): Promise<PackageManifest> =>
                await readManifest(directory),
        ),
    );

    const matches: Array<PackageManifest> = selectPlatformPackages(
        manifests,
        process.platform,
        process.arch,
    );

    let stagedCount: number = 0;

    for (const manifest of matches) {
        const source: string = Path.join(
            npmDir,
            manifest.directory,
            manifest.main,
        );

        // `create-npm-dirs` scaffolds a package directory for every target,
        // but the host build only produces the binary for its own target, so
        // a matching directory can still lack its `.node` file.
        if (!(await exists(source))) continue;

        await stage({
            target: Path.join(bindingDir, manifest.main),
            source,
        });

        stagedCount += 1;
    }

    if (stagedCount === 0) {
        throw new Error(
            `No native binding matches ${process.platform}-${process.arch} under ${npmDir}. Run \`just build-rs\` first.`,
        );
    }
};

const stageWasi = async (): Promise<void> => {
    const wasiDir: string = Path.join(npmDir, "wasm32-wasi");

    const fileNames: Array<string> = [
        "telarel.wasi.cjs",
        "telarel.wasm32-wasi.wasm",
        "wasi-worker.mjs",
    ];

    for (const fileName of fileNames) {
        await stage({
            target: Path.join(bindingDir, fileName),
            source: Path.join(wasiDir, fileName),
        });
    }

    const nodeModulesTarget: string = Path.join(distDir, "node_modules");

    await clear(nodeModulesTarget);

    await Fsp.symlink(
        Path.join(wasiDir, "node_modules"),
        nodeModulesTarget,
        process.platform === "win32" ? "junction" : "dir",
    );

    staged.push(nodeModulesTarget);
};

let exitCleanupRegistered: boolean = false;

const removeTarget = async (target: string): Promise<void> => {
    try {
        await Fsp.rm(target, { force: true, recursive: true });
    } catch (error) {
        if (!isLockError(error)) throw error;
        deferred.push(target);
    }
};

const removeStaged = async (): Promise<void> => {
    const targets: Array<string> = staged.slice();
    staged.length = 0;

    for (const target of targets) {
        await removeTarget(target);
    }

    if (deferred.length === 0 || exitCleanupRegistered) return;

    exitCleanupRegistered = true;

    process.once("exit", (): void => {
        for (const target of deferred) {
            try {
                Fs.rmSync(target, { force: true, recursive: true });
            } catch {
                // Exit-time cleanup runs after the worker processes are closed,
                // but the OS may still hold the mapping briefly; `stage`
                // re-clears each target on the next run.
            }
        }
    });
};

const setup = async (): Promise<Teardown> => {
    const hasDist: boolean = await exists(Path.join(distDir, "index.mjs"));

    if (!hasDist) {
        throw new Error(
            `Missing ${distDir}/index.mjs. Run \`just build-js\` first.`,
        );
    }

    await Fsp.mkdir(bindingDir, { recursive: true });

    if (process.env.NAPI_RS_FORCE_WASI === "error") {
        await stageWasi();
    } else {
        await stageNative();
    }

    return removeStaged;
};

export type { PackageManifest, StageOptions, Teardown };
export {
    exists,
    isLockError,
    selectPlatformPackages,
    stage,
    stageNative,
    stageWasi,
    removeStaged,
    setup,
};
export default setup;
