const WASM_CACHE_MARKERS: ReadonlyArray<string> = [
    "telarel.wasi",
    "binding-wasm32-wasi",
    "@napi-rs/wasm-runtime",
    "@emnapi/",
];

const normalizeCacheKey = (cacheKey: string): string =>
    cacheKey.replaceAll("\\", "/");

const detectWasmModules = (cacheKeys: ReadonlyArray<string>): Array<string> =>
    cacheKeys.filter((cacheKey: string): boolean => {
        const normalized: string = normalizeCacheKey(cacheKey);

        return WASM_CACHE_MARKERS.some((marker: string): boolean =>
            normalized.includes(marker),
        );
    });

const reportBenchmarkBackend = (
    cacheKeys: ReadonlyArray<string>,
    allowWasm: boolean,
): void => {
    const wasmModules: Array<string> = detectWasmModules(cacheKeys);

    if (wasmModules.length > 0 && !allowWasm) {
        throw new Error(
            [
                "The benchmark requires the native binding, but the WASM fallback loaded:",
                ...wasmModules.map((module: string): string => `  ${module}`),
                "Run `just bench-js` for the native benchmark. `just bench-js-wasm` is the deliberate WASM path.",
            ].join("\n"),
        );
    }

    process.stdout.write(
        wasmModules.length > 0
            ? "Backend: WASM\n"
            : "Backend: native\n",
    );
};

export { detectWasmModules, reportBenchmarkBackend };
