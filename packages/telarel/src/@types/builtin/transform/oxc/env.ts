/**
 * Env modules.
 */
type EnvModules = "auto" | "amd" | "umd" | "systemjs" | "commonjs" | "cjs";

/**
 * Babel `preset-env` style options for the oxc `env` layer.
 *
 * Mirrors oxc's `BabelEnvOptions` (camelCase, unknown fields rejected). The
 * deprecated boolean fields are still accepted by the bridge, so they stay;
 * `include` / `exclude` / `useBuiltIns` / `corejs` are opaque on the Rust side
 * and stay unknown.
 */
type OxcEnvOptions = {
    /**
     * Target engines used to determine which transforms are enabled.
     *
     * Accepts browserslist-style strings (`"chrome 80"`, `"chrome >= 80"`),
     * arrays of them, or a map of engine name to version string (`{ chrome:
     * "80" }`).
     */
    targets?: string | Array<string> | Record<string, string>;
    /**
     * Enable Babel's bugfix transforms. Not implemented by oxc.
     */
    bugfixes?: boolean;
    /**
     * Enable spec-compliant transforms over looser output. Not implemented by
     * oxc.
     */
    spec?: boolean;
    /**
     * Enable loose mode for eligible transforms. Not implemented by oxc.
     */
    loose?: boolean;
    /**
     * What module code is generated: a Babel `preset-env` module format or a
     * boolean shorthand.
     */
    modules?: boolean | EnvModules;
    /**
     * Print debugging information while selecting transforms. Not implemented
     * by oxc.
     */
    debug?: boolean;
    /**
     * Force-enable specific transforms. Not implemented by oxc.
     */
    include?: unknown;
    /**
     * Force-disable specific transforms. Not implemented by oxc.
     */
    exclude?: unknown;
    /**
     * Polyfill injection mode. Not implemented by oxc.
     */
    useBuiltIns?: unknown;
    /**
     * `core-js` version or options for polyfill injection. Not implemented by
     * oxc.
     */
    corejs?: unknown;
    /**
     * Force all eligible transforms regardless of targets. Not implemented by
     * oxc.
     */
    forceAllTransforms?: boolean;
    /**
     * Explicit path to config file. Not implemented by oxc.
     */
    configPath?: string;
    /**
     * Skip loading `.browserslistrc`. Not implemented by oxc.
     */
    ignoreBrowserslistConfig?: boolean;
    /**
     * Enable proposal transforms that are shipped in browsers. Not implemented
     * by oxc.
     */
    shippedProposals?: boolean;
};

export type { EnvModules, OxcEnvOptions };
