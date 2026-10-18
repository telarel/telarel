## Next

### Common

- remove `code` from `CompileOptions`; the source is now a call argument
- add `ParseOptions`, `RewriteOptions`, `GenerateOptions`, and `CompileFromAstOptions`
- add `AstResult` and `CodeResult`
- add `SourceMapOptions` with value, path, `inline`, `external`, and `auto` input modes and `inline`, `external`, and `both` output modes
- add the builtin `Parser` and `Generator` types and their options
- add an optional input source map to `Ast`
- add input source-map resolution and output source-map mode application
- change the `file` default to `index.ts`

### Core

- replace `CompileOutput` with `CodeResult`
- add `parse`, `rewrite`, `generate`, and `compile_from_ast` entry points over one shared pipeline
- change the pipeline to run the `options` fixpoint before the driver and to call `start` and `end` exactly once per invocation
- change parser/generator selection to fall back to the builtin after every plugin hook defers
- change stage failures to include the failing hook name and keep plugin names in diagnostics

### Plugin

- rename the `compile_start` and `compile_end` hooks to `start` and `end`
- rename `CompileStartArgs` and `CompileEndArgs` to `StartArgs` and `EndArgs`
- change `prepare` and `finalize` from carried code folds to read-only AST observers
- remove the `PrepareOutput`, `PrepareReturn`, `FinalizeOutput`, and `FinalizeReturn` types
- add first-win `parse` and `generate` hooks
- change the `options` hook to receive `ResolvableOptions` before defaults are resolved

### Plugin Transform

- rename the default Rust plugin name from `builtin:transform` to `telarel_plugin_transform`
- add `TransformPlugin::with_name` to override the reported plugin name

### Binding

- replace `compileStart` and `compileEnd` with `start` and `end`
- rename constructed builtin plugins to their JS-facing names
- add NAPI entry points for `parse`, `rewrite`, `generate`, `compileFromAst`, `compile`, `builtinParse`, and `builtinGenerate`
- add bridges for the new options, source-map types, and `parse` and `generate` hooks
- change AST normalization to `{ program, code, map? }` and accept either an `Ast` or a `Program` plus its source text

### Telarel (Crate)

- replace the `parse_owned` and `codegen` re-exports with the new entry points
- add re-exports of `parse`, `rewrite`, `generate`, `compile_from_ast`, and `compile`
- add re-exports of the new options, result types, source-map types, and plugin hook types

### Telarel (NPM)

- change `compile` to take the source text first: `compile(code, options)`
- rename `CompileResult` to `CodeResult`
- rename the `compileStart` and `compileEnd` hooks to `start` and `end`
- replace `CompileStartArgs` and `CompileEndArgs` with `StartArgs` and `EndArgs`
- remove the `PrepareResult` and `FinalizeResult` exports
- add exports for `parse`, `rewrite`, `generate`, `compileFromAst`, `createParser`, `createGenerator`, and `transform`
- add exports for `Ast`, `AstResult`, `CodeResult`, the per-entry options, and the source-map types

## 0.1.0 (2026-10-10)

initial release
