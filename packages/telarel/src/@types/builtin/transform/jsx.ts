/**
 * The JSX transform runtime.
 */
type JsxRuntime = "classic" | "automatic";

/**
 * Options for the JSX transform, a curated subset of the builtin's fields.
 */
type JsxOptions = {
    /**
     * The JSX runtime to emit. By default, it is `automatic`.
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
