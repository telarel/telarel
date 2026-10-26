import type {
    DecoratorOptions as OxcDecoratorOptions,
    JsxOptions as OxcJsxOptions,
    ReactRefreshOptions as OxcReactRefreshOptions,
    TypeScriptOptions as OxcTypeScriptOptions,
} from "oxc-transform";

import type { OxcCompilerAssumptions } from "#/@types/builtin/transform/oxc/assumptions";
import type { OxcEnvOptions } from "#/@types/builtin/transform/oxc/env";
import type { HelperLoaderMode } from "#/@types/builtin/transform/oxc/helper-loader";

/**
 * The oxc escape hatch: a serializable mirror of oxc's transform options.
 */
type OxcTransformOptions = {
    /**
     * Assumptions for producing smaller output.
     */
    assumptions?: OxcCompilerAssumptions;
    /**
     * The raw oxc TypeScript options.
     */
    typescript?: OxcTypeScriptOptions;
    /**
     * The raw oxc decorator options.
     */
    decorator?: OxcDecoratorOptions;
    /**
     * The raw oxc JSX options. The `refresh` field is narrowed to the object
     * form, because the binding's serde rejects the `boolean` member that npm's
     * type allows.
     */
    jsx?: Omit<OxcJsxOptions, "refresh"> & {
        /**
         * The React Refresh options. Only the object form is accepted.
         */
        refresh?: OxcReactRefreshOptions;
    };
    /**
     * Babel `preset-env` style environment options, mirroring oxc's
     * `BabelEnvOptions`.
     */
    env?: OxcEnvOptions;
    /**
     * Behavior for runtime helpers.
     */
    helperLoader?: {
        /**
         * The strategy used to resolve helper calls. By default, it is
         * `inline`.
         */
        mode?: HelperLoaderMode;
        /**
         * The module to import helper functions from in `runtime` mode. By
         * default, it is `@oxc-project/runtime`.
         */
        moduleName?: string;
    };
};

export type { OxcTransformOptions };
