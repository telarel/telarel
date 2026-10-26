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

- change `Ast::from_source` and `Ast::try_from_source` to accept `Language` and `SourceType` instead of `oxc::span::SourceType`
- change `Ast::source_type` to return `telarel::SourceType` instead of `oxc::span::SourceType`
- change `Ast::with_mut` to require `Program::source_text` to stay content-equal to `Ast::source`
- change `CompileContext.cwd` to `&Path`
- change `CompileError` to carry a message and optional source
- change `CompileOptions`, `CompileFromAstOptions`, and `GenerateOptions` to carry `output`
- change `GenerateOptions` to carry `language`
- change `OutputSourceMapOptions` to carry an optional external map `url`
- change `ParseOptions` and `RewriteOptions` to accept `input_source_map` instead of `source_map`
- change `ResolvableOptions` and `ResolvedOptions` to carry `output`
- change `RewriteOptions.source_type` and `CompileFromAstOptions.source_type` to use a value other than `Unambiguous`, then the AST module kind, then the file extension. `Unambiguous` requests inference.
- change `apply_output_source_map_mode` to accept `OutputSourceMapOptions` and the output file used for the default external map URL
- change the `cwd` parameter of `resolve_input_source_map` to `&Path`
- change the `cwd` field in Rust option structs to `PathBuf`
- change the `file` default to `index.ts`
- change external output mode to append a `sourceMappingURL` default derived from `output.file` when `url` is absent
- remove `CompileError::from_message`; use `CompileError::new` instead
- remove `code` from `CompileOptions`; the source is now a call argument
- remove `InputSourceMapMode::Path`; use `External` with `InputSourceMapOptions.path`, which resolves a relative `path` against the source file's directory instead of `cwd`
- remove `compose_maps`. Use `SourceMap::compose` instead.

#### Features

- add `Ast::language`
- add an optional input source map to `Ast`
- add `AstResult` and `CodeResult`
- add `ErrorSource`
- add `InputSourceMapOptions` and `InputSourceMap::Options` for custom external input map paths
- add `Display`, `FromStr`, and `Language::as_str` for `Language`
- add `OutputOptions` and `ResolvedOutputOptions` for the intended output file
- add `ParseOptions`, `RewriteOptions`, `GenerateOptions`, and `CompileFromAstOptions`
- add the builtin `Parser` and `Generator` types and their options
- add `SourceMapOptions` with value, `inline`, `external`, and `auto` input modes and `inline`, `external`, and `both` output modes
- add input source-map resolution and output source-map mode application

#### Fixes

- change external input source-map lookup to ignore `?query`/`#hash` suffixes in `file`
- change output source-map composition to take `sources`, `sourcesContent`, `sourceRoot`, and `ignoreList` from the input map, and to append generated names as fallbacks

#### Performance

- change `Ast::clone` to share the arena and `Ast::with_mut` to copy on write

### Core

#### Breaking

- change `compile`, `compileFromAst`, and `generate` to append a default external map comment derived from `output.file`
- change `rewrite` and `compile_from_ast` to use a `source_type` value other than `Unambiguous`, then the AST module kind, then the file extension. `Unambiguous` requests inference.
- replace `CompileOutput` with `CodeResult`

#### Features

- add `parse`, `rewrite`, `generate`, and `compile_from_ast` entry points over one shared pipeline

#### Fixes

- change `parse` and `compile` to attach the requested input source map after a winning `parse` hook
- change the pipeline to run the `options` fixpoint before the driver and to call `start` and `end` exactly once per invocation
- change parser/generator selection to fall back to the builtin after every plugin hook defers
- change stage failures to return hook errors unchanged instead of adding stage prefixes
- change a stage failure followed by an `end` hook failure to return the `end` error while still passing the stage error to `end`

### Plugin

#### Breaking

- change `ModuleInfo.source_type` for AST entries to use a `source_type` value other than `Unambiguous`, then the AST module kind, then the file extension. `Unambiguous` requests inference.
- change `PluginContext.cwd` to `&Path`
- change `TransformArgs` to own a cheap `Ast`; it loses its lifetime and `Copy`
- change the `options` hook to receive `ResolvableOptions` before defaults are resolved
- change the `options` fixpoint identity to the plugin instance instead of the plugin name
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

- change `rewrite` and `compileFromAst` to use a `sourceType` value other than `"unambiguous"`, then the AST module kind, then the file extension. `"unambiguous"` requests inference.
- change AST normalization to `{ program, code, map? }` and accept either an `Ast` or a `Program` plus its source text
- change `inputSourceMap` and `sourceMap.input` path strings to `InvalidArg`; use `{ mode: "external", path }`
- change `parse` and `rewrite` JS options to use `inputSourceMap` instead of `sourceMap`
- change `prepare`, `transform`, and `finalize` hook payloads to carry the AST's `code` and optional `map`; transform results are rebuilt from the returned `Ast`
- rename constructed builtin plugins to their JS-facing names
- replace `compileStart` and `compileEnd` with `start` and `end`

#### Features

- add NAPI entry points for `parse`, `rewrite`, `generate`, `compileFromAst`, `compile`, `builtinParse`, and `builtinGenerate`
- add `language` to `JsAst`, `JsAstInput`, and AST hook payloads
- add `language` to `JsGenerateOptions`
- add `output` to the resolved options payload
- add `output.file` to the compile, compileFromAst, and generate options and to the plugin options bag
- add `program.comments` and comment preservation across AST round-trips
- add `{ mode, path }` input and `{ mode, url }` output source-map options to `sourceMap`, and the flat input mode or `{ mode, path }` options to `inputSourceMap`; an input `mode` accepts a source-map value or a literal mode
- add the JS-to-Rust `PluginError` transport
- add structured NAPI rejection data with UTF-16 spans
- add bridges for the new options, source-map types, and `parse` and `generate` hooks

#### Fixes

- change the JS plugin bridge to reuse the adapter for the same plugin object so unchanged plugins do not re-run `options`

### Telarel (Crate)

#### Breaking

- change the re-exported `Ast::from_source` and `Ast::try_from_source` to accept `Language` and `SourceType`
- change the re-exported `Ast::source_type` to return `SourceType`
- change the re-exported `CompileContext.cwd` to `&Path`
- change the re-exported `CompileOptions`, `CompileFromAstOptions`, and `GenerateOptions` to carry `output`
- change the re-exported `InputSourceMapMode` to remove `Path`; use `External` with `InputSourceMapOptions.path`, which resolves a relative `path` against the source file's directory instead of `cwd`
- change the re-exported `ParseOptions` and `RewriteOptions` source-map option to `input_source_map`
- change the re-exported `RewriteOptions.source_type` and `CompileFromAstOptions.source_type` to use a value other than `Unambiguous`, then the AST module kind, then the file extension. `Unambiguous` requests inference.
- change the re-exported `TransformArgs` to own a cheap `Ast` and drop its lifetime and `Copy` implementation
- change the re-exported `cwd` option fields to `PathBuf`
- replace the `parse_owned` and `codegen` re-exports with the new entry points

#### Features

- add re-exports of `parse`, `rewrite`, `generate`, `compile_from_ast`, and `compile`
- add re-exports for `Ast::language` and `GenerateOptions.language`
- add an `InputSourceMapOptions` re-export
- add `OutputOptions` and `ResolvedOutputOptions` re-exports
- add `PluginError` and `ErrorSource` re-exports
- add re-exports of the new options, result types, source-map types, and plugin hook types

### Telarel (NPM)

#### Breaking

- change `compile` to take the source text first: `compile(code, options)`
- change `createGenerator` and `ErrorSource.fromAst` to accept `Ast`
- change `rewrite` and `compileFromAst` to use a `sourceType` value other than `"unambiguous"`, then the AST module kind, then the file extension. `"unambiguous"` requests inference.
- change `Ast` from a plain object to a class with `program`, `source`, and `map` getters
- change `Ast.map` to return `null` when no map is attached
- change `Ast.program` to a deeply read-only view; use `Ast.mutate` to change it
- change `ErrorSource.file`, `ErrorSource.code`, and `ErrorSource.span` from methods to getters
- change `InputSourceMapMode` and `InputSourceMapOptions.mode` to accept a `SourceMap` value or the `"inline"`, `"external"`, and `"auto"` literals; `inputSourceMap` and `sourceMap.input` path strings now reject as `InvalidArg`—use `{ mode: "external", path }`, which resolves a relative `path` against the source file's directory instead of `cwd`
- change `ModuleInfo.sourceType` for `generate` to report the AST module kind, then the file extension.
- change `parse` and `rewrite` options to use `inputSourceMap` instead of `sourceMap`
- change `PrepareArgs`, `FinalizeArgs`, and `GenerateArgs` to carry `Ast` instead of `Program`; `TransformArgs` and `TransformResult` carry `Ast`
- change entry-point rejections to `CompileError`
- change external output mode to append a default `sourceMappingURL` derived from `output.file` when `url` is absent
- change the `options` fixpoint to run once per plugin instance: a same-name replacement runs, an unchanged plugin does not re-run `options`, and duplicate entries are preserved
- remove plain-object `Ast` inputs; construct `new Ast({ program, source })` instead
- remove the `PrepareResult` and `FinalizeResult` exports
- rename `Ast.code` to `Ast.source`
- rename `CompileResult` to `CodeResult`
- rename the `compileStart` and `compileEnd` hooks to `start` and `end`
- replace `CompileStartArgs` and `CompileEndArgs` with `StartArgs` and `EndArgs`

#### Features

- add exports for `parse`, `rewrite`, `generate`, `compileFromAst`, `createParser`, `createGenerator`, and `transform`
- add exports for `Ast`, `AstResult`, `CodeResult`, the per-entry options, and the source-map types
- add `Ast.mutate` for synchronous and asynchronous program mutation
- add `Ast.sourceType` read-through accessor
- add `AstOptions`
- add `AstOptions.language` and `Ast.language`
- add `Comment`, `CommentContent`, `CommentPosition`, `CommentType`, `OxcProgram`, and `Program` types, and comment preservation across AST round-trips
- add `CompileError`, `PluginError`, and `ErrorSource`
- add `CompileError.render` and `PluginError.render` to render a code frame for a message
- add `CompleteOutputOptions` and `OutputOptions` for the intended output file
- add `GenerateOptions.language` and language precedence for `generate`, `createGenerator`, `rewrite`, and `compileFromAst`
- add `InputSourceMapOptions` for custom external input map paths
- add `output.file` to the compile, compileFromAst, and generate options
- add an output `url` option for external `sourceMappingURL` comments

#### Fixes

- change `parse` and `compile` to attach `inputSourceMap` after a winning `parse` hook
- change empty `output.file` and `sourceMap.output.url` to be treated as unset instead of rejected
- change the `options` hook to omit unset `cwd` and `file` instead of sending empty strings
- change external input source-map lookup to ignore `?query`/`#hash` suffixes in `file`

## 0.1.0 (2026-10-10)

initial release
