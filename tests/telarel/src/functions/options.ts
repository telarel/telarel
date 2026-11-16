import type {
    ResolvedGeneratorOptions,
    ResolvedOxcCodegenOptions,
    ResolvedOxcParserOptions,
    ResolvedParserOptions,
} from "telarel";

import { OPTIONS_GENERATOR } from "#/consts/options/generator";
import { OPTIONS_OXC_CODEGEN, OPTIONS_OXC_PARSER } from "#/consts/options/oxc";
import { OPTIONS_PARSER } from "#/consts/options/parser";

/**
 * Builds resolved oxc codegen options from the Telarel defaults and the given
 * overrides.
 */
const resolvedOxcCodegenOptions = (
    options: Partial<ResolvedOxcCodegenOptions> = {},
): ResolvedOxcCodegenOptions => ({
    ...OPTIONS_OXC_CODEGEN,
    ...options,
});

/**
 * Builds resolved oxc parser options from the Telarel defaults and the given
 * overrides.
 */
const resolvedOxcParserOptions = (
    options: Partial<ResolvedOxcParserOptions> = {},
): ResolvedOxcParserOptions => ({
    ...OPTIONS_OXC_PARSER,
    ...options,
});

/**
 * Builds resolved parser options from the integration-test defaults and the
 * given overrides.
 */
const resolvedParserOptions = (
    options: Partial<ResolvedParserOptions> = {},
): ResolvedParserOptions => ({
    ...OPTIONS_PARSER,
    ...options,
});

/**
 * Builds resolved generator options from the integration-test defaults and the
 * given overrides.
 */
const resolvedGeneratorOptions = (
    options: Partial<ResolvedGeneratorOptions> = {},
): ResolvedGeneratorOptions => ({
    ...OPTIONS_GENERATOR,
    ...options,
});

export {
    resolvedGeneratorOptions,
    resolvedOxcCodegenOptions,
    resolvedOxcParserOptions,
    resolvedParserOptions,
};
