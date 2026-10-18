import type { Language, SourceType } from "#/@types/grammar";
import type { SourceMapOptions } from "#/@types/source-map";

/**
 * Resolved options for the builtin parser.
 */
type ParserOptions = {
    /**
     * Current working directory.
     */
    cwd: string;
    /**
     * The file being parsed.
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
