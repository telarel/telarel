import type {
    TransformOptions,
    TransformPlugin,
} from "#/@types/builtin/transform";

/**
 * Creates the builtin transform plugin, which lowers TypeScript and JSX to
 * JavaScript and applies the configured transforms.
 */
const transform = (options?: TransformOptions): TransformPlugin => {
    return { __builtin: true, name: "builtin:transform", options };
};

export type {
    DecoratorOptions as OxcDecoratorOptions,
    JsxOptions as OxcJsxOptions,
    ReactRefreshOptions as OxcReactRefreshOptions,
    TypeScriptOptions as OxcTypeScriptOptions,
} from "oxc-transform";

export type {
    TransformPluginName,
    TransformPlugin,
    TransformOptions,
} from "#/@types/builtin/transform";
export type { DefineValue } from "#/@types/builtin/transform/define";
export type {
    InjectMode,
    InjectObject,
    InjectShorthand,
} from "#/@types/builtin/transform/inject";
export type { OxcCompilerAssumptions } from "#/@types/builtin/transform/oxc/assumptions";
export type {
    EnvModules,
    OxcEnvOptions,
} from "#/@types/builtin/transform/oxc/env";
export type { JsxOptions, JsxRuntime } from "#/@types/builtin/transform/jsx";
export type { OxcTransformOptions } from "#/@types/builtin/transform/oxc";
export type { HelperLoaderMode } from "#/@types/builtin/transform/oxc/helper-loader";
export type {
    EngineTarget,
    EsTarget,
    TransformTarget,
} from "#/@types/builtin/transform/targets";
export type { TypeScriptOptions } from "#/@types/builtin/transform/typescript";

export { transform };
