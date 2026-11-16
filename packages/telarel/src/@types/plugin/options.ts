import type { Omit, Partial } from "ts-vista";

import type { BuiltinPlugin } from "#/@types/builtin";
import type { MaybePromise } from "#/@types/common";
import type {
    GeneratorOptions,
    ResolvedGeneratorStageOptions,
} from "#/@types/generator";
import type { Language, SourceType } from "#/@types/grammar";
import type {
    ParserOptions,
    ResolvedParserStageOptions,
} from "#/@types/parser";
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
 * Output code options.
 */
type CompleteOutputOptions = {
    /**
     * The intended path of the generated code.
     *
     * The compiler does not write the file. A non-empty value is used verbatim.
     * An empty string is treated as unset, the same as an absent `file`.
     * Without the transform plugin, the resolved `file` is used.
     *
     * The resolved path is recorded as `map.file` and derives the default URL
     * for an external source map by appending `.map`.
     */
    file: string;
};

/**
 * Options for the output code.
 *
 * By default, `file` is derived from the input file.
 */
type OutputOptions = Partial<CompleteOutputOptions>;

/**
 * The full, resolved shape of a compile run's options.
 *
 * The source text is deliberately absent. It belongs to the call, never to the
 * options bag.
 */
type CompleteOptions = {
    /**
     * Current working directory.
     *
     * By default, it is `process.cwd()`.
     */
    cwd: string;
    /**
     * The file to be compiled.
     *
     * By default, it is `"index.ts"`.
     */
    file: string;
    /**
     * The grammar of the source code.
     *
     * By default, it is inferred from the input AST or the file extension.
     */
    language: Language;
    /**
     * The module kind of the code.
     *
     * By default, it is resolved from the input AST or the file extension.
     */
    sourceType: SourceType;
    /**
     * The output code options.
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
     * Parser-stage options.
     *
     * By default, it is `undefined`, which applies the parser defaults.
     */
    parser?: ParserOptions;
    /**
     * The input source-map request for the step.
     *
     * `parse` returns an AST and has no output source map.
     *
     * By default, it is `undefined`, which disables input-map lookup.
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
     *
     * By default, it is `undefined`, which disables input-map lookup.
     */
    inputSourceMap?: InputSourceMap;
};

/**
 * Options for `generate`.
 *
 * The module kind is absent. The generated program's own `sourceType` settles
 * it. The grammar precedence is `language`, then the AST language, then the
 * file extension.
 */
type GenerateOptions = Partial<
    Pick<CompleteOptions, "cwd" | "file" | "language" | "plugins">
> & {
    /**
     * Generator-stage options.
     *
     * By default, it is `undefined`, which applies the generator defaults.
     */
    generator?: GeneratorOptions;
    /**
     * The output code options.
     *
     * By default, it is `undefined`.
     */
    output?: OutputOptions;
    /**
     * The source-map inputs and outputs requested for the step.
     *
     * By default, it is `undefined`, which disables source-map input lookup and
     * output delivery.
     */
    sourceMap?: SourceMapOptions;
};

/**
 * Options for `compileFromAst`.
 */
type CompileFromAstOptions = Partial<
    Pick<
        CompleteOptions,
        "cwd" | "file" | "language" | "sourceType" | "plugins"
    >
> & {
    /**
     * Generator-stage options.
     *
     * By default, it is `undefined`, which applies the generator defaults.
     */
    generator?: GeneratorOptions;
    /**
     * The output code options.
     *
     * By default, it is `undefined`.
     */
    output?: OutputOptions;
    /**
     * The source-map inputs and outputs requested for the step.
     *
     * By default, it is `undefined`, which disables source-map input lookup and
     * output delivery.
     */
    sourceMap?: SourceMapOptions;
};

/**
 * Options for `compile`.
 */
type CompileOptions = Partial<
    Pick<
        CompleteOptions,
        "cwd" | "file" | "language" | "sourceType" | "plugins"
    >
> & {
    /**
     * Parser-stage options.
     *
     * By default, it is `undefined`, which applies the parser defaults.
     */
    parser?: ParserOptions;
    /**
     * Generator-stage options.
     *
     * By default, it is `undefined`, which applies the generator defaults.
     */
    generator?: GeneratorOptions;
    /**
     * The output code options.
     *
     * By default, it is `undefined`.
     */
    output?: OutputOptions;
    /**
     * The source-map inputs and outputs requested for the run.
     *
     * By default, it is `undefined`, which disables source-map input lookup and
     * output delivery.
     */
    sourceMap?: SourceMapOptions;
};

/**
 * User options for a compile run.
 */
type Options = Partial<Omit<CompleteOptions, "output" | "sourceMap">> & {
    /**
     * Parser-stage options.
     *
     * By default, it is `undefined`, which applies the parser defaults.
     */
    parser?: ParserOptions;
    /**
     * Generator-stage options.
     *
     * By default, it is `undefined`, which applies the generator defaults.
     */
    generator?: GeneratorOptions;
    /**
     * The output code options.
     *
     * By default, it is `undefined`.
     */
    output?: OutputOptions;
    /**
     * The source-map inputs and outputs requested for the run.
     *
     * By default, it is `undefined`, which disables source-map input lookup and
     * output delivery.
     */
    sourceMap?: SourceMapOptions;
};

/**
 * The options bag an `options` hook observes and returns.
 *
 * Unset `cwd` and `file` are absent rather than empty strings. Defaults are
 * applied only after the options fixpoint settles, so returning the bag
 * unchanged keeps the defaults.
 */
type ResolvableOptions = Partial<
    Omit<CompleteOptions, "output" | "sourceMap">
> & {
    /**
     * Parser-stage options requested for the run.
     *
     * By default, it is `undefined`, which applies the parser defaults.
     */
    parser?: ParserOptions;
    /**
     * Generator-stage options requested for the run.
     *
     * By default, it is `undefined`, which applies the generator defaults.
     */
    generator?: GeneratorOptions;
    /**
     * The output code options.
     *
     * By default, it is `undefined`.
     */
    output?: OutputOptions;
    /**
     * The source-map inputs and outputs requested for the run.
     *
     * By default, it is `undefined`, which disables source-map input lookup and
     * output delivery.
     */
    sourceMap?: SourceMapOptions;
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
type ResolvedOptions = Omit<
    CompleteOptions,
    "output" | "sourceMap" | "plugins"
> & {
    /**
     * Resolved parser-stage settings.
     */
    parser: ResolvedParserStageOptions;
    /**
     * Resolved generator-stage settings.
     */
    generator: ResolvedGeneratorStageOptions;
    /**
     * The output code options.
     */
    output: CompleteOutputOptions;
    /**
     * The source-map inputs and outputs requested for the run.
     */
    sourceMap: SourceMapOptions;
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
