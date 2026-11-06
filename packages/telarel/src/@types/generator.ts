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
     * The file being generated.
     *
     * The builtin generator records it as `map.file`. `createGenerator` has no
     * separate output name. Without an input source map, the file also names
     * the source.
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
