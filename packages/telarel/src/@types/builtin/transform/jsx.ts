/**
 * The JSX transform runtime.
 *
 * `preserve` keeps JSX unchanged. `classic` and `automatic` lower JSX with the
 * corresponding runtime.
 */
type JsxRuntime = "preserve" | "classic" | "automatic";

/**
 * Options for the JSX transform, a curated subset of the builtin's fields.
 */
type JsxOptions = {
    /**
     * The JSX runtime. By default, it is `preserve`, which keeps JSX unchanged.
     *
     * `preserve` overrides every other field in this object.
     */
    runtime?: JsxRuntime;
    /**
     * The module to import JSX helpers from in `classic` mode.
     */
    importSource?: string;
    /**
     * The pragma function for `classic` mode.
     */
    pragma?: string;
    /**
     * The pragma function for fragments in `classic` mode.
     */
    pragmaFrag?: string;
    /**
     * Whether to emit development-friendly JSX helpers.
     */
    development?: boolean;
    /**
     * Whether to emit React Refresh code.
     */
    refresh?: boolean;
};

export type { JsxOptions, JsxRuntime };
