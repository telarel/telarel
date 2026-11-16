import type { Language, SourceType } from "#/@types/grammar";
import type { SourceMapOptions } from "#/@types/sourcemap";

/**
 * Raw oxc parser options, as a camelCase wire mirror.
 *
 * Every field is optional. A set field overrides the matching Telarel default;
 * an unset field keeps it. A curated field on {@link ParserOptions} wins over
 * its matching raw field.
 */
type OxcParserOptions = {
    /**
     * Whether regular-expression literals are parsed into a regex AST.
     */
    parseRegularExpression?: boolean;
    /**
     * Whether `return` is accepted outside a function body.
     */
    allowReturnOutsideFunction?: boolean;
    /**
     * Whether parenthesized expressions stay in the AST.
     */
    preserveParens?: boolean;
    /**
     * Whether V8 intrinsics are accepted.
     */
    allowV8Intrinsics?: boolean;
    /**
     * Whether to precompute hashes for identifier names.
     */
    enableIdentHashes?: boolean;
};

/**
 * The full, effective raw oxc parser options, as seen by `start` and the
 * `parse` hook.
 *
 * Every field is set. It is the result of applying the requested raw overrides
 * over the Telarel defaults, then applying the curated fields.
 */
type ResolvedOxcParserOptions = {
    /**
     * Whether regular-expression literals are parsed into a regex AST.
     */
    parseRegularExpression: boolean;
    /**
     * Whether `return` is accepted outside a function body.
     */
    allowReturnOutsideFunction: boolean;
    /**
     * Whether parenthesized expressions stay in the AST.
     */
    preserveParens: boolean;
    /**
     * Whether V8 intrinsics are accepted.
     */
    allowV8Intrinsics: boolean;
    /**
     * Whether to precompute hashes for identifier names.
     */
    enableIdentHashes: boolean;
};

/**
 * Options for the parser stage.
 *
 * Every field is optional. An unset curated field keeps the matching raw `oxc`
 * value, or the stage default when `oxc` does not set it.
 */
type ParserOptions = {
    /**
     * Whether parenthesized expressions stay in the AST as
     * `ParenthesizedExpression` nodes.
     *
     * By default, it is `false`.
     */
    preserveParens?: boolean;
    /**
     * Whether `return` is accepted outside a function body.
     *
     * By default, it is `false`.
     */
    allowReturnOutsideFunction?: boolean;
    /**
     * Raw oxc parser overrides.
     *
     * A set field overrides the matching Telarel default. A curated field set
     * by the caller wins over its matching raw field.
     */
    oxc?: OxcParserOptions;
};

/**
 * Resolved parser-stage settings, as seen by `start`.
 */
type ResolvedParserStageOptions = {
    /**
     * Whether parenthesized expressions stay in the AST.
     */
    preserveParens: boolean;
    /**
     * Whether `return` is accepted outside a function body.
     */
    allowReturnOutsideFunction: boolean;
    /**
     * The full effective options for the raw oxc parser.
     */
    oxc: ResolvedOxcParserOptions;
};

/**
 * Resolved options for the builtin parser.
 *
 * This is also the `options` payload that a `parse` hook receives.
 */
type ResolvedParserOptions = {
    /**
     * The working directory for relative paths. It anchors a relative `file`
     * and resolves the default path of an external input map.
     */
    cwd: string;
    /**
     * The parsed file's path. It labels diagnostics and locates the external
     * input map by default.
     */
    file: string;
    /**
     * The grammar of the code.
     */
    language: Language;
    /**
     * The module system of the code.
     */
    sourceType: SourceType;
    /**
     * The source-map inputs and outputs requested for the step.
     */
    sourceMap: SourceMapOptions;
    /**
     * Whether parenthesized expressions stay in the AST.
     */
    preserveParens: boolean;
    /**
     * Whether `return` is accepted outside a function body.
     */
    allowReturnOutsideFunction: boolean;
    /**
     * The full effective options for the raw oxc parser.
     */
    oxc: ResolvedOxcParserOptions;
};

export type {
    OxcParserOptions,
    ParserOptions,
    ResolvedOxcParserOptions,
    ResolvedParserOptions,
    ResolvedParserStageOptions,
};
