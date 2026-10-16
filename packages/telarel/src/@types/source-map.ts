import type { SourceMap } from "#/@types/sourcemap";

/**
 * A input source map path, relatively resolved against `cwd`.
 */
type InputSourceMapPath = string & Record<never, never>;

/**
 * How an input source map is obtained for a step.
 *
 * - {@link SourceMap} — used directly
 * - `string` — a path to a map file, relative paths resolve against `cwd`
 * - `"inline"` — read a `//# sourceMappingURL=data:...` comment from the code
 * - `"external"` — read `<cwd>/<file>.map` from disk
 * - `"auto"` — inline comment first, then the external file (best effort)
 */
type InputSourceMapMode =
    | SourceMap
    | InputSourceMapPath
    | "inline"
    | "external"
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
 * An omitted or `null` field is off (no input map lookup) / the default output
 * mode.
 */
type SourceMapOptions = Partial<{
    input: InputSourceMap | null;
    output: OutputSourceMap | null;
}>;

export type {
    InputSourceMapPath,
    InputSourceMapMode,
    InputSourceMap,
    OutputSourceMapMode,
    OutputSourceMapOptions,
    OutputSourceMap,
    SourceMapOptions,
};
