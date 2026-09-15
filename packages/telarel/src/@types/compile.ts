import type { Format, Partial } from "ts-vista";

import type { Plugin } from "#/@types/plugin";
import type { SourceMap } from "#/@types/source-map";

/**
 * User options for a compile run.
 */
type Options = {
    /**
     * Current working directory.
     */
    cwd: string;
    /**
     * The file to be compiled.
     */
    file: string;
    /**
     * The code to be compiled.
     */
    code: string;
};

/**
 * Complete compile options, with no field left unspecified.
 */
type CompleteCompileOptions = Options & {
    /**
     * The plugins to run, in registration order.
     */
    plugins: Plugin[];
};

/**
 * User options for a compile run.
 */
type CompileOptions = Format<Partial<CompleteCompileOptions, "plugins">>;

/**
 * Result of `compile`.
 */
type CompileResult = {
    /**
     * Compiled code.
     */
    code: string;
    /**
     * Source map.
     */
    map: SourceMap;
};

export type { CompleteCompileOptions, CompileOptions, CompileResult, Options };
export type { SourceMap } from "./source-map";
