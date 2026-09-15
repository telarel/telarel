import * as Url from "node:url";

import { defineConfig } from "vitest/config";

import { VITEST_EXCLUDE_DEFAULT } from "./src/const/vitest";

process.env.NAPI_RS_FORCE_WASI = "error";

export default defineConfig({
    resolve: {
        tsconfigPaths: true,
    },
    test: {
        exclude: [...VITEST_EXCLUDE_DEFAULT],
        globalSetup: [
            Url.fileURLToPath(
                new URL("./src/functions/setup.ts", import.meta.url),
            ),
        ],
        logHeapUsage: true,
    },
});
