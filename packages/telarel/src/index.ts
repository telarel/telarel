export type { BuiltinPlugin } from "#/@types/builtin";
export type { CompileOptions, CompileResult } from "#/@types/compile";
export type { Language, SourceType } from "#/@types/grammar";
export type { Plugin } from "#/@types/plugin";
export type {
    Options,
    OptionsArgs,
    ResolvedOptions,
} from "#/@types/plugin/options";
export type {
    CommonPluginContext,
    ModuleInfo,
    PluginContext,
    PluginState,
} from "#/@types/plugin/context";
export type { CompileEndArgs } from "#/@types/plugin/hooks/compile-end";
export type { CompileStartArgs } from "#/@types/plugin/hooks/compile-start";
export type { PostArgs, PostResult } from "#/@types/plugin/hooks/post";
export type { PreArgs, PreResult } from "#/@types/plugin/hooks/pre";
export type {
    TransformArgs,
    TransformResult,
} from "#/@types/plugin/hooks/transform";
export type { SourceMap } from "#/@types/sourcemap";

export { compile } from "#/functions/compile";
