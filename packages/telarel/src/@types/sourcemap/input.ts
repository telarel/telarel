import type { Partial } from "ts-vista";

import type { SourceMap } from "#/@types/sourcemap/map";

/**
 * How an input source map is obtained for a step.
 *
 * - {@link SourceMap}: used directly
 * - `inline`: read a `//# sourceMappingURL=data:...` comment from the code
 * - `external`: read an external map. A `?query` or `#hash` suffix is ignored.
 * - `auto`: the inline comment first, then the external map (best effort)
 */
type InputSourceMapMode = SourceMap | "inline" | "external" | "auto";

/**
 * Fully specified input source-map options.
 *
 * `mode` is required and `path` is optional. `path` is used by `external` and
 * the `auto` fallback. It is ignored when `mode` is a `SourceMap`.
 */
type CompleteInputSourceMapOptions = {
    /**
     * How the map is obtained.
     */
    mode: InputSourceMapMode;
    /**
     * Path to a local external map file.
     *
     * Absolute paths are used as-is. Relative paths resolve against the source
     * file's directory. Remote maps are never fetched. Pass a {@link SourceMap}
     * value instead.
     *
     * Used by `external` and the `auto` fallback. Ignored by other modes.
     */
    path: string;
};

/**
 * Options for the input source map.
 */
type InputSourceMapOptions = Partial<CompleteInputSourceMapOptions, "path">;

/**
 * The input source-map request: a bare input mode or full input options.
 */
type InputSourceMap = InputSourceMapMode | InputSourceMapOptions;

export type {
    InputSourceMapMode,
    CompleteInputSourceMapOptions,
    InputSourceMapOptions,
    InputSourceMap,
};
