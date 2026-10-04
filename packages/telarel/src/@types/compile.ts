import type { Format, Partial } from "ts-vista";

import type { CompleteOptions } from "#/@types/plugin/options";
import type { SourceMap } from "#/@types/sourcemap";

/**
 * Options for `compile` function.
 */
type CompileOptions = Format<
    Partial<CompleteOptions, "cwd" | "language" | "sourceType" | "plugins">
>;

/**
 * The result of `compile` function.
 */
type CompileResult = {
    /**
     * The compiled code.
     */
    code: string;
    /**
     * The source map for the compiled code.
     */
    map: SourceMap;
};

export type { CompileOptions, CompileResult };
