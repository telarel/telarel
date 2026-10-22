import type { Omit, Partial } from "ts-vista";

import type { BuiltinPlugin } from "#/@types/builtin";
import type { MaybePromise } from "#/@types/common";
import type { Language, SourceType } from "#/@types/grammar";
import type { Plugin } from "#/@types/plugin";
import type { InputSourceMap, SourceMapOptions } from "#/@types/source-map";

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
>;

/**
 * Options for `compileFromAst`.
 */
type CompileFromAstOptions = Partial<
    Pick<
        CompleteOptions,
        "cwd" | "file" | "language" | "sourceType" | "sourceMap" | "plugins"
    >
>;

/**
 * Options for `compile`.
 */
type CompileOptions = Partial<
    Pick<
        CompleteOptions,
        "cwd" | "file" | "language" | "sourceType" | "sourceMap" | "plugins"
    >
>;

/**
 * User options for a compile run.
 */
type Options = Partial<CompleteOptions>;

/**
 * The options bag an `options` hook observes and returns.
 *
 * Unset `cwd` and `file` are absent rather than empty strings. Defaults are
 * applied only after the options fixpoint settles, so returning the bag
 * unchanged keeps the defaults.
 */
type ResolvableOptions = Partial<CompleteOptions>;

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
    CompileFromAstOptions,
    CompileOptions,
    FalsyPlugin,
    GenerateOptions,
    Options,
    OptionsArgs,
    ParseOptions,
    PluginOption,
    ResolvableOptions,
    ResolvedOptions,
    RewriteOptions,
};
