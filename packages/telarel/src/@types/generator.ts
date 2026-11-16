import type { SourceMapOptions } from "#/@types/sourcemap";

/**
 * The indentation style for generated code.
 */
type IndentStyle = "space" | "tab";

/**
 * An oxc legal-comment policy.
 *
 * `linked` writes the given URL in the legal-comment banner.
 */
type OxcLegalComment =
    | "none"
    | "inline"
    | "eof"
    | "external"
    | {
          /**
           * The URL that a linked legal comment points to.
           */
          linked: string;
      };

/**
 * Raw oxc comment-printing overrides.
 *
 * Every field is optional. A set field overrides the matching Telarel default;
 * an unset field keeps it.
 */
type OxcCommentOptions = {
    /**
     * Whether normal comments are printed.
     */
    normal?: boolean;
    /**
     * Whether JSDoc comments are printed.
     */
    jsdoc?: boolean;
    /**
     * Whether annotation comments are printed.
     */
    annotation?: boolean;
    /**
     * The legal-comment policy.
     */
    legal?: OxcLegalComment;
};

/**
 * Raw oxc codegen options, as a camelCase wire mirror.
 *
 * Every field is optional. A set field overrides the matching Telarel default;
 * an unset field keeps it. A curated field on {@link GeneratorOptions} wins over
 * its matching raw field. `sourceMapPath` is owned by Telarel and is not
 * accepted here.
 */
type OxcCodegenOptions = {
    /**
     * Whether string literals use single quotes.
     */
    singleQuote?: boolean;
    /**
     * Whether generated code is minified.
     */
    minify?: boolean;
    /**
     * Whether non-ASCII characters are escaped.
     */
    asciiOnly?: boolean;
    /**
     * The comment-printing overrides.
     */
    comments?: OxcCommentOptions;
    /**
     * The raw indentation character.
     */
    indentChar?: IndentStyle;
    /**
     * The number of characters per indentation level.
     */
    indentWidth?: number;
    /**
     * The indentation depth applied to the whole output.
     */
    initialIndent?: number;
};

/**
 * The full, effective raw oxc comment-printing options.
 */
type ResolvedOxcCommentOptions = {
    /**
     * Whether normal comments are printed.
     */
    normal: boolean;
    /**
     * Whether JSDoc comments are printed.
     */
    jsdoc: boolean;
    /**
     * Whether annotation comments are printed.
     */
    annotation: boolean;
    /**
     * The legal-comment policy.
     */
    legal: OxcLegalComment;
};

/**
 * The full, effective raw oxc codegen options, as seen by `start` and the
 * `generate` hook.
 *
 * Every field is set. It is the result of applying the requested raw overrides
 * over the Telarel defaults, then applying the curated fields.
 */
type ResolvedOxcCodegenOptions = {
    /**
     * Whether string literals use single quotes.
     */
    singleQuote: boolean;
    /**
     * Whether generated code is minified.
     */
    minify: boolean;
    /**
     * Whether non-ASCII characters are escaped.
     */
    asciiOnly: boolean;
    /**
     * The full comment-printing options.
     */
    comments: ResolvedOxcCommentOptions;
    /**
     * The raw indentation character.
     */
    indentChar: IndentStyle;
    /**
     * The number of characters per indentation level.
     */
    indentWidth: number;
    /**
     * The indentation depth applied to the whole output.
     */
    initialIndent: number;
};

/**
 * Options for the generator stage.
 *
 * Every field is optional. An unset curated field keeps the matching raw `oxc`
 * value, or the stage default when `oxc` does not set it.
 */
type GeneratorOptions = {
    /**
     * Whether string literals use single quotes instead of double quotes.
     *
     * By default, it is `false`.
     */
    singleQuote?: boolean;
    /**
     * Whether non-ASCII characters are escaped in generated output.
     *
     * By default, it is `false`.
     */
    asciiOnly?: boolean;
    /**
     * The indentation style for generated code.
     *
     * By default, it is `"space"`.
     */
    indentStyle?: IndentStyle;
    /**
     * The number of characters per indentation level.
     *
     * By default, it is `2`.
     */
    indentWidth?: number;
    /**
     * Whether comments are printed in generated code.
     *
     * By default, it is `true`. `false` disables every comment category. `true`
     * restores the default policy only when the raw comments are fully
     * disabled; explicitly set raw categories are preserved.
     *
     * `minify: true` disables comments only when neither this field nor the raw
     * comments are set.
     */
    comments?: boolean;
    /**
     * Whether generated code is minified.
     *
     * By default, it is `false`.
     */
    minify?: boolean;
    /**
     * Raw oxc codegen overrides.
     *
     * A set field overrides the matching Telarel default. A curated field set
     * by the caller wins over its matching raw field. `sourceMapPath` is owned
     * by Telarel and is not accepted here.
     */
    oxc?: OxcCodegenOptions;
};

/**
 * Resolved generator-stage settings, as seen by `start`.
 */
type ResolvedGeneratorStageOptions = {
    /**
     * Whether string literals use single quotes.
     */
    singleQuote: boolean;
    /**
     * Whether non-ASCII characters are escaped.
     */
    asciiOnly: boolean;
    /**
     * The resolved indentation style.
     */
    indentStyle: IndentStyle;
    /**
     * The resolved indentation width.
     */
    indentWidth: number;
    /**
     * Whether any comment category is printed.
     */
    comments: boolean;
    /**
     * Whether generated code is minified.
     */
    minify: boolean;
    /**
     * The full effective options for the raw oxc codegen.
     */
    oxc: ResolvedOxcCodegenOptions;
};

/**
 * Resolved options for the builtin generator.
 *
 * This is also the `options` payload that a `generate` hook receives.
 */
type ResolvedGeneratorOptions = {
    /**
     * The working directory. It anchors the source file used to resolve a
     * relative `sourceMap.input.path`.
     */
    cwd: string;
    /**
     * The file being generated.
     *
     * The builtin generator records it as `map.file`. `createGenerator` has no
     * separate output name. Without an input source map, the file also names
     * the source.
     */
    file: string;
    /**
     * The source-map inputs and outputs requested for the step.
     *
     * `createGenerator` resolves `input` and applies `output` delivery. A
     * `generate` hook sees both request fields unset, because the pipeline
     * resolved the input before the hook and owns output delivery.
     */
    sourceMap: SourceMapOptions;
    /**
     * Whether string literals use single quotes.
     */
    singleQuote: boolean;
    /**
     * Whether non-ASCII characters are escaped.
     */
    asciiOnly: boolean;
    /**
     * The resolved indentation style.
     */
    indentStyle: IndentStyle;
    /**
     * The resolved indentation width.
     */
    indentWidth: number;
    /**
     * Whether any comment category is printed.
     */
    comments: boolean;
    /**
     * Whether generated code is minified.
     */
    minify: boolean;
    /**
     * The full effective options for the raw oxc codegen.
     */
    oxc: ResolvedOxcCodegenOptions;
};

export type {
    GeneratorOptions,
    IndentStyle,
    OxcCodegenOptions,
    OxcCommentOptions,
    OxcLegalComment,
    ResolvedGeneratorOptions,
    ResolvedGeneratorStageOptions,
    ResolvedOxcCodegenOptions,
    ResolvedOxcCommentOptions,
};
