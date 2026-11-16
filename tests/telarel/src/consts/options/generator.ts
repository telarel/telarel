import type {
    ResolvedGeneratorOptions,
    ResolvedGeneratorStageOptions,
} from "telarel";

import { OPTIONS_OXC_CODEGEN } from "#/consts/options/oxc";

/**
 * The resolved generator-stage defaults used by the integration tests.
 */
const OPTIONS_GENERATOR_STAGE: ResolvedGeneratorStageOptions = {
    singleQuote: false,
    asciiOnly: false,
    indentStyle: "space",
    indentWidth: 2,
    comments: true,
    minify: false,
    oxc: OPTIONS_OXC_CODEGEN,
};

/**
 * The resolved generator defaults used by the integration tests.
 */
const OPTIONS_GENERATOR: ResolvedGeneratorOptions = {
    ...OPTIONS_GENERATOR_STAGE,
    cwd: "/repo",
    file: "index.ts",
    sourceMap: {},
};

export { OPTIONS_GENERATOR, OPTIONS_GENERATOR_STAGE };
