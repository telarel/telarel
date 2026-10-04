import type { Format, Omit, Partial } from "ts-vista";

import type { BuiltinPlugin } from "#/@types/builtin";
import type { MaybePromise } from "#/@types/common";
import type { Language, SourceType } from "#/@types/grammar";
import type { Plugin } from "#/@types/plugin";

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

type CompleteOptions = {
    /**
     * Current working directory. By default, it is `process.cwd()`.
     *
     * Inside an `options` hook it reflects the value carried so far, and may be
     * empty when a previous `options` hook omitted it; it is re-resolved to the
     * process working directory before `compileStart` and later hooks.
     */
    cwd: string;
    /**
     * The file to be compiled. By default, it is `"index.js"`.
     */
    file: string;
    /**
     * The code to be compiled. By default, it is `""`.
     */
    code: string;
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
     * Plugins to run.
     *
     * By default, it is `[]`. Duplicates are allowed.
     */
    plugins: Array<PluginOption>;
};

/**
 * User options for a compile run.
 */
type Options = Format<Partial<CompleteOptions>>;

type OptionsArgs = {
    /**
     * The options bag carried so far.
     */
    options: Options;
};

/**
 * Resolved options, as seen by `compileStart` and later hooks.
 */
type ResolvedOptions = Format<
    Omit<CompleteOptions, "plugins"> & {
        /**
         * The settled plugin list, after the options fixpoint.
         */
        plugins: Array<string>;
    }
>;

export type {
    CompleteOptions,
    FalsyPlugin,
    Options,
    OptionsArgs,
    PluginOption,
    ResolvedOptions,
};
