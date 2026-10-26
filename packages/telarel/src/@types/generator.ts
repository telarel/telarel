import type { SourceMapOptions } from "#/@types/sourcemap";

/**
 * Resolved options for the builtin generator.
 */
type GeneratorOptions = {
    /**
     * The working directory. Builtin codegen does not use it.
     */
    cwd: string;
    /**
     * The output file path recorded in the generated source map.
     */
    file: string;
    /**
     * The source-map inputs and outputs requested for the step.
     */
    sourceMap: SourceMapOptions;
};

export type { GeneratorOptions };
