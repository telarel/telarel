import type { SourceMapOptions } from "#/@types/sourcemap";

/**
 * Resolved options for the builtin generator.
 */
type GeneratorOptions = {
    /**
     * Current working directory.
     */
    cwd: string;
    /**
     * The file being generated.
     */
    file: string;
    /**
     * The source-map inputs and outputs requested for the step.
     */
    sourceMap: SourceMapOptions;
};

export type { GeneratorOptions };
