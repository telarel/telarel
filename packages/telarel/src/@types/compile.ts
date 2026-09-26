import type { Format, Partial } from "ts-vista";

import type { BuiltinPlugin } from "#/@types/builtin";
import type { Plugin } from "#/@types/plugin";
import type { SourceMap } from "#/@types/source-map";

/**
 * The language of the source code.
 */
type Language = "js" | "ts" | "dts" | "jsx" | "tsx";

/**
 * The module system / execution mode of the source code.
 *
 * - `script` — classic non-module script
 * - `commonjs` — CommonJS (`require` / `module.exports`)
 * - `module` — ES Module (`import` / `export`)
 * - `unambiguous` - the parser infers from the statements
 */
type SourceType = "script" | "commonjs" | "module" | "unambiguous";

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
    /**
     * The language of the source code.
     */
    language: Language;
    /**
     * The module system of the source code.
     */
    sourceType: SourceType;
};

/**
 * Complete compile options, with no field left unspecified.
 */
type CompleteCompileOptions = Options & {
    /**
     * The plugins to run, in registration order.
     */
    plugins: Array<Plugin | BuiltinPlugin>;
};

/**
 * User options for a compile run.
 */
type CompileOptions = Format<
    Partial<
        CompleteCompileOptions,
        "cwd" | "language" | "sourceType" | "plugins"
    >
>;

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

export type {
    Language,
    SourceType,
    CompleteCompileOptions,
    CompileOptions,
    CompileResult,
    Options,
};
