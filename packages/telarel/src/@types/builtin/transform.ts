import type {
    CompilerAssumptions,
    DecoratorOptions,
    JsxOptions as OxcJsxOptions,
    ReactRefreshOptions,
    TypeScriptOptions as OxcTypeScriptOptions,
} from "oxc-transform";

import type { BuiltinPlugin } from "#/@types/builtin";
import type { BindingBuiltinPluginName } from "#/binding";

/**
 * A supported JavaScript language target, by ES edition.
 */
type EsTarget =
    | "es2015"
    | "es2016"
    | "es2017"
    | "es2018"
    | "es2019"
    | "es2020"
    | "es2021"
    | "es2022"
    | "es2023"
    | "es2024"
    | "es2025"
    | "es2026"
    | "esnext";

/**
 * A browser or runtime engine version target, keyed by engine name.
 */
type EngineTarget =
    | { chrome: number }
    | { deno: number }
    | { edge: number }
    | { firefox: number }
    | { ios: number }
    | { node: number }
    | { opera: number }
    | { safari: number }
    | { samsung: number }
    | { electron: number };

/**
 * A compilation target: an ES edition, a fixed runtime, or an engine version.
 */
type TransformTarget = EsTarget | "hermes" | "rhino" | EngineTarget;

/**
 * The JSX transform runtime.
 */
type JsxRuntime = "classic" | "automatic";

/**
 * The strategy used to resolve runtime helpers.
 */
type HelperLoaderMode = "inline" | "runtime" | "external";

/**
 * Options for the JSX transform, a curated subset of the builtin's fields.
 */
type JsxOptions = {
    /**
     * The JSX runtime to emit.
     *
     * By default, it is `automatic`.
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

/**
 * Options for the TypeScript transform, a curated subset of the builtin's
 * fields.
 */
type TypeScriptOptions = {
    /**
     * Whether to keep value imports that only import types.
     */
    onlyRemoveTypeImports?: boolean;
    /**
     * Whether to leave TypeScript namespaces unelided.
     */
    allowNamespaces?: boolean;
    /**
     * The identifier treated as the JSX pragma.
     */
    jsxPragma?: string;
    /**
     * The identifier treated as the JSX fragment pragma.
     */
    jsxPragmaFrag?: string;
    /**
     * Whether to simplify enum reads that oxc can constant-fold.
     */
    optimizeEnums?: boolean;
};

/**
 * The oxc escape hatch: a serializable mirror of oxc's transform options.
 *
 * The binding accepts a subset of oxc's shape — `assumptions`, `typescript`,
 * `decorator`, `jsx`, `env`, `helperLoader` — and rejects unknown fields,
 * including the Babel-only `cwd`, `plugins`, and `proposals`.
 */
type OxcTransformOptions = {
    /**
     * Assumptions for producing smaller output.
     */
    assumptions?: CompilerAssumptions;
    /**
     * The raw oxc TypeScript options.
     */
    typescript?: OxcTypeScriptOptions;
    /**
     * The raw oxc decorator options.
     */
    decorator?: DecoratorOptions;
    /**
     * The raw oxc JSX options, with `refresh` narrowed to the object form: the
     * binding's serde rejects the `boolean` member npm's type allows.
     */
    jsx?: Omit<OxcJsxOptions, "refresh"> & {
        refresh?: ReactRefreshOptions;
    };
    /**
     * Babel `preset-env` style environment options, mirroring oxc's
     * `BabelEnvOptions`.
     */
    env?: OxcEnvOptions;
    /**
     * Behaviour for runtime helpers, mirroring oxc's `HelperLoaderOptions`.
     */
    helperLoader?: {
        /**
         * The module to import helper functions from.
         *
         * By default, it is `@oxc-project/runtime`.
         */
        moduleName?: string;
        /**
         * Strategy used to resolve helper calls.
         *
         * By default, it is `runtime`.
         */
        mode?: HelperLoaderMode;
    };
};

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

/**
 * Options for the builtin transform plugin.
 */
type TransformOptions = {
    /**
     * The compilation targets.
     */
    targets?: Array<TransformTarget>;
    /**
     * The JSX transform options.
     */
    jsx?: JsxOptions;
    /**
     * The TypeScript transform options.
     */
    typescript?: TypeScriptOptions;
    /**
     * The oxc escape hatch.
     */
    oxc?: OxcTransformOptions;
};

/**
 * The transform plugin's name.
 */
type TransformPluginName = `${BindingBuiltinPluginName.Transform}`;

/**
 * The transform plugin's descriptor.
 */
type TransformPlugin = BuiltinPlugin<TransformPluginName, TransformOptions>;

export type {
    TransformPluginName,
    TransformPlugin,
    EngineTarget,
    EsTarget,
    EnvModules,
    HelperLoaderMode,
    JsxOptions,
    JsxRuntime,
    OxcTransformOptions,
    TransformOptions,
    TransformTarget,
    TypeScriptOptions,
};
