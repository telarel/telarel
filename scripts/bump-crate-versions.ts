import type { ConsolaInstance } from "consola";

import * as Fsp from "node:fs/promises";
import * as Path from "node:path";
import * as Process from "node:process";
import * as Url from "node:url";

import { parse, patch } from "@decimalturn/toml-patch";
import { createConsola } from "consola";

const __dirname: string = Path.dirname(Url.fileURLToPath(import.meta.url));

const workspaceRoot: string = Path.resolve(__dirname, "..");

const MANIFEST_PATH: string = Path.resolve(workspaceRoot, "Cargo.toml");

const CRATE_KEY_PATTERN: RegExp = /^telarel(?:_|$)/;

const consola: ConsolaInstance = createConsola({
    formatOptions: {
        date: false,
    },
});

type WorkspaceDependency = {
    version: string;
};

type WorkspaceManifest = {
    workspace: {
        package: {
            version: string;
        };
        dependencies: Record<string, WorkspaceDependency>;
    };
};

const readVersionArgument = (argv: ReadonlyArray<string>): string => {
    const value: string | undefined = argv[2];

    if (value === void 0 || value.length === 0) {
        throw new Error(
            "Usage: node ./scripts/bump-crate-versions.ts <version>",
        );
    }

    return value;
};

type UpdateVersionsOptions = {
    manifest: WorkspaceManifest;
    version: string;
};

const withUpdatedVersions = (
    options: UpdateVersionsOptions,
): WorkspaceManifest => {
    const dependencies: Record<string, WorkspaceDependency> =
        Object.fromEntries(
            Object.entries(options.manifest.workspace.dependencies).map(
                (
                    entry: [string, WorkspaceDependency],
                ): [string, WorkspaceDependency] => {
                    const [name, dependency]: [string, WorkspaceDependency] =
                        entry;

                    if (!CRATE_KEY_PATTERN.test(name)) {
                        return [name, dependency];
                    }

                    return [name, { ...dependency, version: options.version }];
                },
            ),
        );

    return {
        ...options.manifest,
        workspace: {
            ...options.manifest.workspace,
            package: {
                ...options.manifest.workspace.package,
                version: options.version,
            },
            dependencies,
        },
    };
};

const version: string = readVersionArgument(Process.argv);

const source: string = await Fsp.readFile(MANIFEST_PATH, "utf-8");

const manifest: WorkspaceManifest = parse(source) as WorkspaceManifest;

const previousVersion: string = manifest.workspace.package.version;

const updated: string = patch(
    source,
    withUpdatedVersions({ manifest, version }),
);

if (updated === source) {
    consola.info(`Rust crates already at ${version}.`);
} else {
    await Fsp.writeFile(MANIFEST_PATH, updated, "utf-8");

    consola.success(`Bumped Rust crates: ${previousVersion} -> ${version}.`);
}
