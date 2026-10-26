import type { Partial } from "ts-vista";

/**
 * Where the output source map is delivered.
 *
 * - `inline`: append a `//# sourceMappingURL=data:...` comment
 * - `external`: return the map separately with a comment
 * - `both`: append the data-URI comment and also return the map
 */
type OutputSourceMapMode = "inline" | "external" | "both";

/**
 * Fully specified output source-map options.
 *
 * `mode` and `url` are both required. `url` is used by `external`.
 */
type CompleteOutputSourceMapOptions = {
    /**
     * How the map is delivered.
     */
    mode: OutputSourceMapMode;
    /**
     * URL written verbatim into the appended `sourceMappingURL` comment.
     *
     * Used only by `external`. Ignored by other modes. An empty string is
     * treated as unset, the same as an absent `url`.
     */
    url: string;
};

/**
 * Options for the output source map.
 */
type OutputSourceMapOptions = Partial<CompleteOutputSourceMapOptions, "url">;

/**
 * The output source-map request, either a bare mode or full options.
 */
type OutputSourceMap = OutputSourceMapMode | OutputSourceMapOptions;

export type {
    OutputSourceMapMode,
    CompleteOutputSourceMapOptions,
    OutputSourceMapOptions,
    OutputSourceMap,
};
