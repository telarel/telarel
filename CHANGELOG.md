## Next

### Dependencies

#### Rust

- change `base64` to `~0.23.0`
- change `oxc`, `oxc_compat`, `oxc_transformer_plugins`, and `oxc_traverse` to `~0.154.0`
- change `oxc_estree_codec` to `~0.2.1`

#### JavaScript

- change `@oxc-project/runtime` to `0.154.0`
- change `@oxc-project/types` to `~0.154.0`
- change `oxc-transform` to `~0.154.0`

### Common

#### Breaking

- change `Ast::with_mut` to require `Program::source_text` to stay content-equal to `Ast::source`
- change `CompileContext.cwd` to `&Path`
- change `CompileError` to carry a message and optional source
- change `ParseOptions` and `RewriteOptions` to accept `input_source_map` instead of `source_map`
- change the `cwd` field in Rust option structs to `PathBuf`
- change the `file` default to `index.ts`
- change the `cwd` parameter of `resolve_input_source_map` to `&Path`
- remove `CompileError::from_message`; use `CompileError::new` instead
- remove `code` from `CompileOptions`; the source is now a call argument

#### Features

- add an optional input source map to `Ast`
- add `AstResult` and `CodeResult`
- add `ErrorSource`
- add `Display`, `FromStr`, and `Language::as_str` for `Language`
- add `ParseOptions`, `RewriteOptions`, `GenerateOptions`, and `CompileFromAstOptions`
- add the builtin `Parser` and `Generator` types and their options
- add `SourceMapOptions` with value, path, `inline`, `external`, and `auto` input modes and `inline`, `external`, and `both` output modes
- add input source-map resolution and output source-map mode application

#### Fixes

- change external input source-map lookup to ignore `?query`/`#hash` suffixes in `file`

#### Performance

- change `Ast::clone` to share the arena and `Ast::with_mut` to copy on write

### Core

#### Breaking

- replace `CompileOutput` with `CodeResult`

#### Features

- add `parse`, `rewrite`, `generate`, and `compile_from_ast` entry points over one shared pipeline

#### Fixes

- change the pipeline to run the `options` fixpoint before the driver and to call `start` and `end` exactly once per invocation
- change parser/generator selection to fall back to the builtin after every plugin hook defers
- change stage failures to return hook errors unchanged instead of adding stage prefixes
- change a stage failure followed by an `end` hook failure to return the `end` error while still passing the stage error to `end`

### Plugin

#### Breaking

- change `PluginContext.cwd` to `&Path`
- change `TransformArgs` to own a cheap `Ast`; it loses its lifetime and `Copy`
- change the `options` hook to receive `ResolvableOptions` before defaults are resolved
- change `prepare` and `finalize` from carried code folds to read-only AST observers
- change transform hooks to mutate through `Ast::with_mut` and return the AST; `None` still leaves the carried AST untouched
- change every fallible hook to return `Result<_, PluginError>`
- remove the `PrepareOutput`, `PrepareReturn`, `FinalizeOutput`, and `FinalizeReturn` types
- rename `CompileStartArgs` and `CompileEndArgs` to `StartArgs` and `EndArgs`
- rename the `compile_start` and `compile_end` hooks to `start` and `end`

#### Features

- add `PluginError`
- add first-win `parse` and `generate` hooks

#### Fixes

- change hook errors to keep the plugin's message and source without adding the plugin or hook name

### Plugin Transform

#### Breaking

- rename the default Rust plugin name from `builtin:transform` to `telarel_plugin_transform`

#### Features

- add `TransformPlugin::with_name` to override the reported plugin name

### Binding

#### Breaking

- change AST normalization to `{ program, code, map? }` and accept either an `Ast` or a `Program` plus its source text
- change `prepare`, `transform`, and `finalize` hook payloads to carry the AST's `code` and optional `map`; transform results are rebuilt from the returned `Ast`
- change `parse` and `rewrite` JS options to use `inputSourceMap` instead of `sourceMap`
- rename constructed builtin plugins to their JS-facing names
- replace `compileStart` and `compileEnd` with `start` and `end`

#### Features

- add NAPI entry points for `parse`, `rewrite`, `generate`, `compileFromAst`, `compile`, `builtinParse`, and `builtinGenerate`
- add the JS-to-Rust `PluginError` transport
- add structured NAPI rejection data with UTF-16 spans
- add bridges for the new options, source-map types, and `parse` and `generate` hooks

### Telarel (Crate)

#### Breaking

- change the re-exported `CompileContext.cwd` to `&Path`
- change the re-exported `cwd` option fields to `PathBuf`
- change the re-exported `ParseOptions` and `RewriteOptions` source-map option to `input_source_map`
- change the re-exported `TransformArgs` to own a cheap `Ast` and drop its lifetime and `Copy` implementation
- replace the `parse_owned` and `codegen` re-exports with the new entry points

#### Features

- add re-exports of `parse`, `rewrite`, `generate`, `compile_from_ast`, and `compile`
- add re-exports of the new options, result types, source-map types, and plugin hook types
- add `PluginError` and `ErrorSource` re-exports

### Telarel (NPM)

#### Breaking

- change `compile` to take the source text first: `compile(code, options)`
- change `createGenerator` and `ErrorSource.fromAst` to accept `Ast`
- change `parse` and `rewrite` options to use `inputSourceMap` instead of `sourceMap`
- change `Ast` from a plain object to a class with `program`, `source`, and `map` getters
- change `Ast.map` to return `null` when no map is attached
- change `Ast.program` to a deeply read-only view; use `Ast.mutate` to change it
- change `ErrorSource.file`, `ErrorSource.code`, and `ErrorSource.span` from methods to getters
- change `PrepareArgs`, `FinalizeArgs`, and `GenerateArgs` to carry `Ast` instead of `Program`; `TransformArgs` and `TransformResult` carry `Ast`
- change entry-point rejections to `CompileError`
- remove plain-object `Ast` inputs; construct `new Ast({ program, source })` instead
- remove the `PrepareResult` and `FinalizeResult` exports
- rename `Ast.code` to `Ast.source`
- rename `CompileResult` to `CodeResult`
- rename the `compileStart` and `compileEnd` hooks to `start` and `end`
- replace `CompileStartArgs` and `CompileEndArgs` with `StartArgs` and `EndArgs`

#### Features

- add `AstOptions`
- add `Ast.mutate` for synchronous and asynchronous program mutation
- add exports for `parse`, `rewrite`, `generate`, `compileFromAst`, `createParser`, `createGenerator`, and `transform`
- add exports for `Ast`, `AstResult`, `CodeResult`, the per-entry options, and the source-map types
- add `CompileError`, `PluginError`, and `ErrorSource`
- add `CompileError.render` and `PluginError.render` to render a code frame for a message

#### Fixes

- change external input source-map lookup to ignore `?query`/`#hash` suffixes in `file`

## 0.1.0 (2026-10-10)

initial release
