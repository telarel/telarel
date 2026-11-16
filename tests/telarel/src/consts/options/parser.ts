import type {
    ResolvedParserOptions,
    ResolvedParserStageOptions,
} from "telarel";

import { OPTIONS_OXC_PARSER } from "#/consts/options/oxc";

/**
 * The resolved parser-stage defaults used by the integration tests.
 */
const OPTIONS_PARSER_STAGE: ResolvedParserStageOptions = {
    preserveParens: false,
    allowReturnOutsideFunction: false,
    oxc: OPTIONS_OXC_PARSER,
};

/**
 * The resolved parser defaults used by the integration tests.
 */
const OPTIONS_PARSER: ResolvedParserOptions = {
    ...OPTIONS_PARSER_STAGE,
    cwd: "/repo",
    file: "index.ts",
    language: "ts",
    sourceType: "unambiguous",
    sourceMap: {},
};

export { OPTIONS_PARSER, OPTIONS_PARSER_STAGE };
