import type { PackageManifest } from "#/functions/setup";

import { describe, expect, it } from "vitest";

import { isLockError, selectPlatformPackages } from "#/functions/setup";

const manifest = (
    directory: string,
    main: string,
    os?: Array<string>,
    cpu?: Array<string>,
): PackageManifest => ({
    directory,
    main,
    os,
    cpu,
});

const errorWithCode = (code: string): NodeJS.ErrnoException => {
    const error: NodeJS.ErrnoException = new Error(code);
    error.code = code;
    return error;
};

const NPM_PACKAGES: ReadonlyArray<PackageManifest> = [
    manifest(
        "darwin-arm64",
        "telarel.darwin-arm64.node",
        ["darwin"],
        ["arm64"],
    ),
    manifest("darwin-x64", "telarel.darwin-x64.node", ["darwin"], ["x64"]),
    manifest("linux-x64-gnu", "telarel.linux-x64-gnu.node", ["linux"], ["x64"]),
    manifest(
        "linux-x64-musl",
        "telarel.linux-x64-musl.node",
        ["linux"],
        ["x64"],
    ),
    manifest(
        "linux-arm64-gnu",
        "telarel.linux-arm64-gnu.node",
        ["linux"],
        ["arm64"],
    ),
    manifest(
        "win32-x64-msvc",
        "telarel.win32-x64-msvc.node",
        ["win32"],
        ["x64"],
    ),
    manifest(
        "win32-arm64-msvc",
        "telarel.win32-arm64-msvc.node",
        ["win32"],
        ["arm64"],
    ),
    manifest("wasm32-wasi", "telarel.wasi.cjs", void 0, ["wasm32"]),
];

const mainsFor = (platform: string, arch: string): Array<string> =>
    selectPlatformPackages(NPM_PACKAGES, platform, arch).map(
        (entry: PackageManifest): string => entry.main,
    );

describe("selectPlatformPackages", (): void => {
    it("selects the single macOS arm64 binding", (): void => {
        expect(mainsFor("darwin", "arm64")).toEqual([
            "telarel.darwin-arm64.node",
        ]);
    });

    it("selects both gnu and musl linux-x64 bindings", (): void => {
        expect(mainsFor("linux", "x64")).toEqual([
            "telarel.linux-x64-gnu.node",
            "telarel.linux-x64-musl.node",
        ]);
    });

    it("selects the windows-x64 msvc binding", (): void => {
        expect(mainsFor("win32", "x64")).toEqual([
            "telarel.win32-x64-msvc.node",
        ]);
    });

    it("excludes the wasm package from native selection", (): void => {
        expect(mainsFor("linux", "x64")).not.toContain("telarel.wasi.cjs");
        expect(mainsFor("win32", "x64")).not.toContain("telarel.wasi.cjs");
    });

    it("treats missing constraints as wildcards", (): void => {
        const wildcard: ReadonlyArray<PackageManifest> = [
            manifest("wildcard", "telarel.wildcard.node"),
        ];

        const selected: Array<PackageManifest> = selectPlatformPackages(
            wildcard,
            "linux",
            "x64",
        );

        expect(selected).toHaveLength(1);
        expect(selected[0]?.main).toBe("telarel.wildcard.node");
    });

    it("returns nothing for an unsupported platform", (): void => {
        expect(selectPlatformPackages(NPM_PACKAGES, "freebsd", "x64")).toEqual(
            [],
        );
    });
});

describe("isLockError", (): void => {
    it("matches transient Windows lock codes", (): void => {
        expect(isLockError(errorWithCode("EPERM"))).toBe(true);
        expect(isLockError(errorWithCode("EBUSY"))).toBe(true);
    });

    it("ignores other error codes", (): void => {
        expect(isLockError(errorWithCode("ENOENT"))).toBe(false);
        expect(isLockError(errorWithCode("EACCES"))).toBe(false);
    });

    it("ignores non-error values", (): void => {
        expect(isLockError("EPERM")).toBe(false);
        expect(isLockError(void 0)).toBe(false);
    });
});
