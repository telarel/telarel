import { defineConfig } from "@apst/oxfmt";
import { IGNORE_PATTERNS_DEFAULT } from "@apst/oxfmt/constants/ignore-patterns";

export default defineConfig({
    ignorePatterns: [
        ...IGNORE_PATTERNS_DEFAULT,
        // Rust
        "target/**",
        // Transform plugin runtime helpers
        "crates/plugin_transform/src/helpers/runtime/**",
        // Binding
        "packages/telarel/src/binding/**",
        // Docs Index
        "docs/content/index.mdx",
    ],
});
