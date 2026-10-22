import type { Omit, Partial } from "ts-vista";

import type { BuiltinPlugin } from "#/@types/builtin";
import type { MaybePromise } from "#/@types/common";
import type { Language, SourceType } from "#/@types/grammar";
import type { Plugin } from "#/@types/plugin";
import type { SourceMapOptions } from "#/@types/sourcemap";
import type { InputSourceMap } from "#/@types/sourcemap/input";

/**
 * A conditional plugin slot that is skipped.
 */
type FalsyPlugin = false | null | undefined;

/**
 * A plugin list entry.
 */
type PluginOption = MaybePromise<
    Plugin | BuiltinPlugin | FalsyPlugin | PluginOption[]
>;

/**
 * Output code settings.
 */
type CompleteOutputOptions = {
    /**
     * The intended path of the generated code.
     *
     * The compiler does not write the file. The resolved path derives the
     * default external source-map URL by appending `.map`.
     *
     * An empty string is treated as unset, same as an absent `file`.
     *
     * By default, it is the resolved `file`.
     */
    file: string;
};

/**
 * Options for the output code.
 */
type OutputOptions = Partial<CompleteOutputOptions>;

/**
 * The full, resolved shape of a compile run's options.
 *
 * The source text is deliberately absent: it belongs to the call, never to the
 * options bag.
 */
type CompleteOptions = {
    /**
     * Current working directory. By default, it is `process.cwd()`.
     */
    cwd: string;
    /**
     * The file to be compiled. By default, it is `"index.ts"`.
     */
    file: string;
    /**
     * The grammar of the code. By default, it is inferred from the file
     * extension.
     */
    language: Language;
    /**
     * The module system of the code. By default, it is kept resolved from the
     * grammar.
     */
    sourceType: SourceType;
    /**
     * The output code settings.
     */
    output: CompleteOutputOptions;
    /**
     * The source-map inputs and outputs requested for the run.
     */
    sourceMap: SourceMapOptions;
    /**
     * Plugins to run.
     *
     * By default, it is `[]`. Duplicates are allowed.
     */
    plugins: Array<PluginOption>;
};

/**
 * Options for `parse`.
 */
type ParseOptions = Partial<
    Pick<
        CompleteOptions,
        "cwd" | "file" | "language" | "sourceType" | "plugins"
    >
> & {
    /**
     * The input source-map request for the step.
     *
     * `parse` returns an AST and has no output source map.
     */
    inputSourceMap?: InputSourceMap;
};

/**
 * Options for `rewrite`.
 */
type RewriteOptions = Partial<
    Pick<
        CompleteOptions,
        "cwd" | "file" | "language" | "sourceType" | "plugins"
    >
> & {
    /**
     * The input source-map request for the step.
     *
     * `rewrite` returns an AST and has no output source map.
     */
    inputSourceMap?: InputSourceMap;
};

/**
 * Options for `generate`.
 *
 * The grammar and module kind are absent: the generated program's own
 * `sourceType` settles the module kind.
 */
type GenerateOptions = Partial<
    Pick<CompleteOptions, "cwd" | "file" | "sourceMap" | "plugins">
> & {
    /**
     * The output code settings.
     */
    output?: OutputOptions;
};

/**
 * Options for `compileFromAst`.
 */
type CompileFromAstOptions = Partial<
    Pick<
        CompleteOptions,
        "cwd" | "file" | "language" | "sourceType" | "sourceMap" | "plugins"
    >
> & {
    /**
     * The output code settings.
     */
    output?: OutputOptions;
};

/**
 * Options for `compile`.
 */
type CompileOptions = Partial<
    Pick<
        CompleteOptions,
        "cwd" | "file" | "language" | "sourceType" | "sourceMap" | "plugins"
    >
> & {
    /**
     * The output code settings.
     */
    output?: OutputOptions;
};

/**
 * User options for a compile run.
 */
type Options = Partial<Omit<CompleteOptions, "output">> & {
    /**
     * The output code settings.
     */
    output?: OutputOptions;
};

/**
 * The options bag an `options` hook observes and returns.
 *
 * Unset `cwd` and `file` are absent rather than empty strings. Defaults are
 * applied only after the options fixpoint settles, so returning the bag
 * unchanged keeps the defaults.
 */
type ResolvableOptions = Partial<Omit<CompleteOptions, "output">> & {
    /**
     * The output code settings.
     */
    output?: OutputOptions;
};

/**
 * Arguments passed to an `options` hook.
 */
type OptionsArgs = {
    /**
     * The options bag carried so far.
     */
    options: ResolvableOptions;
};

/**
 * Resolved options, as seen by `start` and later hooks.
 */
type ResolvedOptions = Omit<CompleteOptions, "plugins"> & {
    /**
     * The settled plugin list, after the options fixpoint.
     */
    plugins: Array<string>;
};

export type {
    CompleteOptions,
    CompleteOutputOptions,
    CompileFromAstOptions,
    CompileOptions,
    FalsyPlugin,
    GenerateOptions,
    Options,
    OptionsArgs,
    OutputOptions,
    ParseOptions,
    PluginOption,
    ResolvableOptions,
    ResolvedOptions,
    RewriteOptions,
};
