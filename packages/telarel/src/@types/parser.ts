import type { Language, SourceType } from "#/@types/grammar";
import type { SourceMapOptions } from "#/@types/sourcemap";

/**
 * Resolved options for the builtin parser.
 */
type ParserOptions = {
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
};

export type { ParserOptions };
