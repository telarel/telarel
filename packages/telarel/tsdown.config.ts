import { defineConfig } from "@apst/tsdown";
import { cjsPreset, dtsPreset, esmPreset } from "@apst/tsdown/presets";

export default defineConfig(
    {
        entry: [
            "./src/index.ts",
            "./src/ast.ts",
            "./src/walker.ts",
            "./src/plugins/*.ts",
        ],
        platform: "node",
        deps: {
            onlyBundle: false,
            neverBundle: true,
            alwaysBundle: ["oxc-transform"],
        },
        unbundle: true,
    },
    [
        esmPreset(),
        cjsPreset(),
        dtsPreset({
            presetOptions: {
                performanceMode: true,
            },
        }),
    ],
);
