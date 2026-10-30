import type { SourceMapOptions } from "#/@types/sourcemap";

/**
 * Resolved options for the builtin generator.
 */
type GeneratorOptions = {
    /**
     * The working directory. It anchors the source file used to resolve a
     * relative `sourceMap.input.path`.
     */
    cwd: string;
    /**
     * The output file path recorded in the generated source map.
     */
    file: string;
    /**
     * The source-map inputs and outputs requested for the step.
     *
     * `createGenerator` resolves `input` and applies `output` delivery. A
     * `generate` hook sees both request fields unset, because the pipeline
     * resolved the input before the hook and owns output delivery.
     */
    sourceMap: SourceMapOptions;
};

export type { GeneratorOptions };
