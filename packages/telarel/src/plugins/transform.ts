import type {
    TransformOptions,
    TransformPlugin,
} from "#/@types/builtin/transform";

const transform = (options?: TransformOptions): TransformPlugin => {
    return { __builtin: true, name: "builtin:transform", options };
};

export type {
    TransformPluginName,
    EsTarget,
    EngineTarget,
    EnvModules,
    HelperLoaderMode,
    TransformTarget,
    JsxRuntime,
    JsxOptions,
    TypeScriptOptions,
    OxcTransformOptions,
    TransformOptions,
    TransformPlugin,
} from "#/@types/builtin/transform";
export { transform };
