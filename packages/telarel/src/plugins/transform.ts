import type {
    TransformOptions,
    TransformPlugin,
} from "#/@types/builtin/transform";

const transform = (options?: TransformOptions): TransformPlugin => {
    return { __builtin: true, name: "builtin:transform", options };
};

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
