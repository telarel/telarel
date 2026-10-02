import type {
    CompilerAssumptions,
    DecoratorOptions,
    JsxOptions as OxcJsxOptions,
    ReactRefreshOptions,
    TypeScriptOptions as OxcTypeScriptOptions,
} from "oxc-transform";

import type { OxcEnvOptions } from "#/@types/builtin/transform/oxc/env";
import type { HelperLoaderMode } from "#/@types/builtin/transform/oxc/helper-loader";

/**
 * The oxc escape hatch: a serializable mirror of oxc's transform options.
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
     * Behaviour for runtime helpers.
     */
    helperLoader?: {
        /**
         * Strategy used to resolve helper calls. By default, it is `inline`.
         */
        mode?: HelperLoaderMode;
        /**
         * The module to import helper functions from, work in `runtime` mode.
         * By default, it is `@oxc-project/runtime`.
         */
        moduleName?: string;
    };
};

export type { OxcTransformOptions };
