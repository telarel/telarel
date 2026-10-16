export type { Ast, AstResult, CodeResult } from "#/@types/ast";
export type { BuiltinPlugin } from "#/@types/builtin";
export type { CompileOptions, CompileResult } from "#/@types/compile";
export type { GeneratorOptions } from "#/@types/generator";
export type { Language, SourceType } from "#/@types/grammar";
export type { ParserOptions } from "#/@types/parser";
export type { ObjectHook, Plugin, PluginOrder } from "#/@types/plugin";
export type {
    FalsyPlugin,
    Options,
    OptionsArgs,
    PluginOption,
    ResolvableOptions,
    ResolvedOptions,
} from "#/@types/plugin/options";
export type {
    CommonPluginContext,
    ModuleInfo,
    PluginContext,
} from "#/@types/plugin/context";
export type { EndArgs } from "#/@types/plugin/hooks/end";
export type { FinalizeArgs } from "#/@types/plugin/hooks/finalize";
export type { GenerateArgs } from "#/@types/plugin/hooks/generate";
export type { ParseArgs } from "#/@types/plugin/hooks/parse";
export type { PrepareArgs } from "#/@types/plugin/hooks/prepare";
export type { StartArgs } from "#/@types/plugin/hooks/start";
export type {
    TransformArgs,
    TransformResult,
} from "#/@types/plugin/hooks/transform";
export type {
    InputSourceMap,
    InputSourceMapMode,
    OutputSourceMap,
    OutputSourceMapMode,
    OutputSourceMapOptions,
    SourceMapOptions,
} from "#/@types/source-map";
export type { SourceMap } from "#/@types/sourcemap";

export { compile } from "#/functions/compile";
