import type { SourceMap } from "#/@types/sourcemap";

/**
 * How an input source map is obtained for a step.
 *
 * - {@link SourceMap} — used directly
 * - `string` — a path to a map file, relative paths resolve against `cwd`
 * - `"inline"` — read a `//# sourceMappingURL=data:...` comment from the code
 * - `"external"` — read `<cwd>/<file>.map` from disk
 * - `"auto"` — inline comment first, then the external file (best effort)
 *
 * The literal modes are documentation only: `string` absorbs them for typing.
 */
type InputSourceMapMode =
    | SourceMap
    | string /* path */
    // oxlint-disable-next-line typescript/no-redundant-type-constituents
    | "inline"
    // oxlint-disable-next-line typescript/no-redundant-type-constituents
    | "external"
    // oxlint-disable-next-line typescript/no-redundant-type-constituents
    | "auto";

/**
 * The input source-map request.
 */
type InputSourceMap = InputSourceMapMode;

/**
 * Where the output source map is delivered.
 *
 * - `"inline"` — append a `//# sourceMappingURL=data:...` comment
 * - `"external"` — return the map separately, no comment (default)
 * - `"both"` — append the comment and also return the map
 */
type OutputSourceMapMode = "inline" | "external" | "both";

/**
 * Options for the output source map.
 */
type OutputSourceMapOptions = {
    /**
     * How the map is delivered.
     */
    mode: OutputSourceMapMode;
};

/**
 * The output source-map request, either a bare mode or full options.
 */
type OutputSourceMap = OutputSourceMapMode | OutputSourceMapOptions;

/**
 * The source-map inputs and outputs requested for a step.
 *
 * An omitted field is off (no input map lookup) / the default output mode.
 */
type SourceMapOptions = Partial<{
    /**
     * The input source-map request; omitted means no input map lookup.
     */
    input: InputSourceMap;
    /**
     * The output source-map request; omitted means the default output mode.
     */
    output: OutputSourceMap;
}>;

export type {
    InputSourceMapMode,
    InputSourceMap,
    OutputSourceMapMode,
    OutputSourceMapOptions,
    OutputSourceMap,
    SourceMapOptions,
};
