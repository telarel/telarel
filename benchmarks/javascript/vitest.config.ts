import * as Url from "node:url";

import { defineConfig } from "vitest/config";

import { VITEST_EXCLUDE_DEFAULT } from "../../tests/telarel/src/const/vitest";

export default defineConfig({
    resolve: {
        tsconfigPaths: true,
    },
    test: {
        exclude: [...VITEST_EXCLUDE_DEFAULT],
        globalSetup: [
            Url.fileURLToPath(new URL("./src/setup.ts", import.meta.url)),
        ],
        logHeapUsage: true,
    },
});
