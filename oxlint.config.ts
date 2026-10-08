import { defineConfig } from "@apst/oxlint";
import { IGNORE_PATTERNS_DEFAULT } from "@apst/oxlint/constants/ignore-patterns";
import { commonPreset } from "@apst/oxlint/presets/common";
import { jsxPreset } from "@apst/oxlint/presets/jsx";
import { nodePreset } from "@apst/oxlint/presets/node";
import { reactPreset } from "@apst/oxlint/presets/react";
import { vitestPreset } from "@apst/oxlint/presets/vitest";

export default defineConfig(
    {
        ignorePatterns: [
            ...IGNORE_PATTERNS_DEFAULT,
            // Rust
            "crates/**",
            "target/**",
            // Transform plugin runtime helpers
            "crates/plugin_transform/src/helpers/runtime/**",
            // Binding
            "packages/telarel/src/binding/**",
        ],
        options: {
            typeAware: true,
            typeCheck: true,
        },
    },
    [
        // Foundation
        commonPreset(),
        // Environment
        nodePreset(),
        // Framework
        jsxPreset(),
        reactPreset(),
        // Test
        vitestPreset(),
    ],
);
