import type {
    ResolvedOxcCodegenOptions,
    ResolvedOxcCommentOptions,
    ResolvedOxcParserOptions,
} from "telarel";

/**
 * The Telarel default resolved oxc comment-printing options.
 */
const OPTIONS_OXC_COMMENT = {
    normal: true,
    jsdoc: true,
    annotation: true,
    legal: "inline",
} as const satisfies ResolvedOxcCommentOptions;

/**
 * The Telarel default resolved oxc codegen options.
 */
const OPTIONS_OXC_CODEGEN: ResolvedOxcCodegenOptions = {
    singleQuote: false,
    minify: false,
    asciiOnly: false,
    comments: OPTIONS_OXC_COMMENT,
    indentChar: "space",
    indentWidth: 2,
    initialIndent: 0,
};

/**
 * The Telarel default resolved oxc parser options.
 */
const OPTIONS_OXC_PARSER: ResolvedOxcParserOptions = {
    parseRegularExpression: false,
    allowReturnOutsideFunction: false,
    preserveParens: false,
    allowV8Intrinsics: false,
    enableIdentHashes: true,
};

export { OPTIONS_OXC_CODEGEN, OPTIONS_OXC_COMMENT, OPTIONS_OXC_PARSER };
