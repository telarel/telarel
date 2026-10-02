import type { Format, Omit, Partial } from "ts-vista";

import type { BuiltinPlugin } from "#/@types/builtin";
import type { Language, SourceType } from "#/@types/grammar";
import type { Plugin } from "#/@types/plugin";

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
     * Plugins to run, in order.
     *
     * By default, it is `[]`. Duplicates are allowed.
     */
    plugins: Array<Plugin | BuiltinPlugin>;
};

/**
 * User options for a compile run.
 */
type Options = Format<Partial<CompleteOptions>>;

type OptionsArgs = {
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

export type { CompleteOptions, Options, OptionsArgs, ResolvedOptions };
