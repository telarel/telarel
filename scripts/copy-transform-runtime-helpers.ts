import type { ConsolaInstance } from "consola";

import * as Fsp from "node:fs/promises";
import * as Path from "node:path";
import { fileURLToPath } from "node:url";

import { createConsola } from "consola";

const __dirname: string = Path.dirname(fileURLToPath(import.meta.url));

const workspaceRoot: string = Path.resolve(__dirname, "..");

const crateDir: string = Path.resolve(
    workspaceRoot,
    "crates",
    "plugin_transform",
);

const RUNTIME_PACKAGE_NAME: string = "@oxc-project/runtime";

const HELPER_NAME_PATTERN: RegExp = /name: "([^"]+)"/g;

const consola: ConsolaInstance = createConsola({
    formatOptions: {
        date: false,
    },
});

const PACKAGE_JSON_PATH: string = Path.resolve(workspaceRoot, "package.json");

const HELPERS_DIR_ESM: string = Path.resolve(
    workspaceRoot,
    "node_modules",
    "@oxc-project",
    "runtime",
    "src",
    "helpers",
    "esm",
);

const SOURCES_PATH: string = Path.resolve(
    crateDir,
    "src",
    "helpers",
    "sources.rs",
);

const OUTPUT_DIR: string = Path.resolve(crateDir, "src", "helpers", "runtime");

type PackageJson = {
    devDependencies: Partial<Record<string, string>>;
};

const readRuntimeVersion = async (): Promise<string> => {
    const content: string = await Fsp.readFile(PACKAGE_JSON_PATH, "utf-8");

    const pkg: PackageJson = JSON.parse(content) as PackageJson;

    const version: string | undefined =
        pkg.devDependencies[RUNTIME_PACKAGE_NAME];

    if (version === void 0) {
        throw new Error(
            `${RUNTIME_PACKAGE_NAME} is not a devDependency in ${PACKAGE_JSON_PATH}.`,
        );
    }

    return version.replace(/^[~^]/, "");
};

const collectHelperNames = async (): Promise<ReadonlyArray<string>> => {
    const content: string = await Fsp.readFile(SOURCES_PATH, "utf-8");

    const names: Array<string> = [];

    for (const match of content.matchAll(HELPER_NAME_PATTERN)) {
        const name: string | undefined = match[1];

        if (name !== void 0) {
            names.push(name);
        }
    }

    if (names.length === 0) {
        throw new Error(`No helper names found in ${SOURCES_PATH}.`);
    }

    return names;
};

type CopyOptions = {
    names: ReadonlyArray<string>;
    inputDir: string;
    outputDir: string;
};

const copyHelpers = async (
    options: CopyOptions,
): Promise<ReadonlyArray<string>> => {
    await Fsp.mkdir(options.outputDir, { recursive: true });

    const copiedFiles: Array<string> = [];

    for (const name of options.names) {
        const source: string = Path.resolve(options.inputDir, `${name}.js`);
        const target: string = Path.resolve(options.outputDir, `${name}.js`);

        await Fsp.copyFile(source, target);

        copiedFiles.push(`${name}.js`);
    }

    return copiedFiles;
};

type PruneOptions = {
    names: ReadonlyArray<string>;
    outputDir: string;
};

const pruneStaleHelpers = async (
    options: PruneOptions,
): Promise<ReadonlyArray<string>> => {
    const expected: ReadonlySet<string> = new Set(
        options.names.map((name: string): string => `${name}.js`),
    );

    const removedFiles: Array<string> = [];

    for (const entry of await Fsp.readdir(options.outputDir)) {
        if (!entry.endsWith(".js") || expected.has(entry)) {
            continue;
        }

        await Fsp.rm(Path.resolve(options.outputDir, entry));

        removedFiles.push(entry);
    }

    return removedFiles;
};

const runtimeVersion: string = await readRuntimeVersion();

const helperNames: ReadonlyArray<string> = await collectHelperNames();

consola.info(
    `Copying ${helperNames.length} helper(s) from ${RUNTIME_PACKAGE_NAME}@${runtimeVersion}...`,
);

const copied: ReadonlyArray<string> = await copyHelpers({
    names: helperNames,
    inputDir: HELPERS_DIR_ESM,
    outputDir: OUTPUT_DIR,
});

const removed: ReadonlyArray<string> = await pruneStaleHelpers({
    names: helperNames,
    outputDir: OUTPUT_DIR,
});

if (removed.length > 0) {
    consola.info(
        `Removed ${removed.length} stale helper(s): ${removed.join(", ")}`,
    );
}

consola.success(
    `Copied ${copied.length} helper(s) into ${Path.relative(workspaceRoot, OUTPUT_DIR)}.`,
);
