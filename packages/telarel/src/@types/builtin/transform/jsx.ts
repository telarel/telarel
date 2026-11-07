/**
 * How the transform handles JSX.
 *
 * - `preserve`: keeps JSX unchanged
 * - `classic`: lowers JSX to `pragma` calls
 * - `automatic`: lowers JSX to helpers imported from `importSource`
 */
type JsxRuntime = "preserve" | "classic" | "automatic";

/**
 * Options for the JSX transform, a curated subset of the builtin's fields.
 */
type JsxOptions = {
    /**
     * The JSX runtime.
     *
     * By default, it is `preserve`, which keeps JSX unchanged.
     */
    runtime?: JsxRuntime;
    /**
     * The module to import JSX helpers from in `automatic` mode.
     *
     * By default, it is `react`.
     */
    importSource?: string;
    /**
     * The pragma function for `classic` mode.
     *
     * By default, it is `React.createElement`.
     */
    pragma?: string;
    /**
     * The pragma function for fragments in `classic` mode.
     *
     * By default, it is `React.Fragment`.
     */
    pragmaFrag?: string;
    /**
     * Whether to emit development-friendly JSX helpers.
     *
     * By default, it is `false`.
     */
    development?: boolean;
    /**
     * Whether to emit React Refresh code.
     *
     * By default, it is `false`.
     */
    refresh?: boolean;
};

export type { JsxOptions, JsxRuntime };
