import { defineConfig } from "vitest/config";

process.env.NAPI_RS_FORCE_WASI = "error";

export default defineConfig({
    resolve: {
        tsconfigPaths: true,
    },
    test: {
        logHeapUsage: true,
    },
});
