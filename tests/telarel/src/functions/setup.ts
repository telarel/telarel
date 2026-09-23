import * as Fsp from "node:fs/promises";
import * as Path from "node:path";
import * as Url from "node:url";

type StageOptions = {
    target: string;
    source: string;
};

type Teardown = () => Promise<void>;

const testDir: string = Path.dirname(Url.fileURLToPath(import.meta.url));
const repoDir: string = Path.resolve(testDir, "..", "..", "..", "..");
const pkgDir: string = Path.resolve(repoDir, "packages", "telarel");
const distDir: string = Path.join(pkgDir, "dist");
const npmDir: string = Path.join(pkgDir, "npm");

const staged: Array<string> = [];

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
    await Fsp.symlink(options.source, options.target, "file");
    staged.push(options.target);
};

const stageNative = async (): Promise<void> => {
    const fileName: string = `telarel.${process.platform}-${process.arch}.node`;

    const npmDirs: Array<string> = await Fsp.readdir(npmDir);

    for (const npmDirName of npmDirs) {
        const source: string = Path.join(npmDir, npmDirName, fileName);
        if (await exists(source)) {
            await stage({
                target: Path.join(distDir, fileName),
                source,
            });
            return;
        }
    }

    throw new Error(
        `Native binding "${fileName}" not found under ${npmDir}. Run \`just build-rs\` first.`,
    );
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
            target: Path.join(distDir, fileName),
            source: Path.join(wasiDir, fileName),
        });
    }

    const nodeModulesTarget: string = Path.join(distDir, "node_modules");

    await clear(nodeModulesTarget);

    await Fsp.symlink(
        Path.join(wasiDir, "node_modules"),
        nodeModulesTarget,
        "dir",
    );

    staged.push(nodeModulesTarget);
};

const removeStaged = async (): Promise<void> => {
    for (const target of staged) await Fsp.rm(target, { force: true });
    staged.length = 0;
};

const setup = async (): Promise<Teardown> => {
    const hasDist: boolean = await exists(Path.join(distDir, "index.mjs"));

    if (!hasDist) {
        throw new Error(
            `Missing ${distDir}/index.mjs. Run \`just build-js\` first.`,
        );
    }

    if (process.env.NAPI_RS_FORCE_WASI === "error") {
        await stageWasi();
    } else {
        await stageNative();
    }

    return removeStaged;
};

export type { StageOptions, Teardown };
export { exists, stage, stageNative, stageWasi, removeStaged, setup };
export default setup;
