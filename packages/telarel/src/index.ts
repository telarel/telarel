export type { AstResult, CodeResult } from "#/@types/ast";
export type { BuiltinPlugin } from "#/@types/builtin";
export type { DeepReadonly } from "#/@types/common";
export type { GeneratorOptions } from "#/@types/generator";
export type { Language, SourceType } from "#/@types/grammar";
export type { ParserOptions } from "#/@types/parser";
export type { ObjectHook, Plugin, PluginOrder } from "#/@types/plugin";
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
    CompileFromAstOptions,
    CompileOptions,
    FalsyPlugin,
    GenerateOptions,
    Options,
    OptionsArgs,
    ParseOptions,
    PluginOption,
    ResolvableOptions,
    ResolvedOptions,
    RewriteOptions,
} from "#/@types/plugin/options";
export type { SourceMap } from "#/@types/sourcemap";
export type {
    InputSourceMap,
    InputSourceMapMode,
    OutputSourceMap,
    OutputSourceMapMode,
    OutputSourceMapOptions,
    SourceMapOptions,
} from "#/@types/source-map";
export type { AstOptions } from "#/classes/ast";
export type {
    CompileErrorOptions,
    CompleteCompileErrorOptions,
} from "#/errors/compile";
export type { PluginErrorOptions } from "#/errors/plugin";
export type { Span } from "#/errors/source/span";
export type { Generator } from "#/functions/generator";
export type { Parser } from "#/functions/parser";

export { Ast } from "#/classes/ast";
export { CompileError } from "#/errors/compile";
export { PluginError } from "#/errors/plugin";
export { ErrorSource } from "#/errors/source";
export { compileFromAst } from "#/functions/compile-from-ast";
export { compile } from "#/functions/compile";
export { generate } from "#/functions/generate";
export { createGenerator } from "#/functions/generator";
export { parse } from "#/functions/parse";
export { createParser } from "#/functions/parser";
export { rewrite } from "#/functions/rewrite";
export { transform } from "#/plugins/transform";
