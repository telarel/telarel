# Rust Usage

Public contract: [Rust API](https://telarel.github.io/apis/rust/compile).

## Dependencies and Imports

```sh
cargo add telarel
cargo add telarel_plugin_transform
```

```rust
use std::path::PathBuf;

use telarel::{
    Ast, AstResult, CodeResult, CompileError, CompileFromAstOptions,
    CompileOptions, GenerateOptions, Generator, GeneratorOptions, Language,
    ParseOptions, Parser, ParserOptions, Plugin, RewriteOptions,
    SharedPluginable, SourceMapOptions, SourceType, compile, compile_from_ast,
    generate, parse, rewrite,
};
use telarel_plugin_transform::{
    TransformOptions, TransformPlugin, TransformTarget,
};
```

All entry points are async. Use a runtime such as Tokio:

```toml
[dev-dependencies]
tokio = { version = "1", features = ["macros", "rt"] }
```

```rust
#[tokio::main(flavor = "current_thread")]
async fn main() {
    // ...
}
```

Every entry-point option struct also accepts `cwd: Option<PathBuf>` (default: process working directory); `CompileOptions` lists it.

## `compile`

```rust
pub async fn compile(
    code: String,
    options: CompileOptions,
    plugins: Vec<SharedPluginable>,
) -> Result<CodeResult, CompileError>
```

Runs parse, prepare, transform, finalize, and generate.

```rust
let result: CodeResult = compile(
    String::from("const x: number = 1;"),
    CompileOptions {
        file: Some(String::from("src/index.ts")),
        ..CompileOptions::default()
    },
    vec![Plugin::new_shared(TransformPlugin::with_options(
        TransformOptions {
            targets: vec![TransformTarget::Es2015],
            ..TransformOptions::default()
        },
    ))],
)
.await?;
```

### `CompileOptions`

Every field is optional.

| Field         | Type                       | Default             | Description                                   |
| ------------- | -------------------------- | ------------------- | --------------------------------------------- |
| `cwd`         | `Option<PathBuf>`          | process working dir | Root for relative paths.                      |
| `file`        | `Option<String>`           | `index.ts`          | Input file. The extension infers the grammar. |
| `language`    | `Option<Language>`         | inferred            | Grammar. Overrides extension inference.       |
| `source_type` | `Option<SourceType>`       | inferred            | Module kind.                                  |
| `output`      | `Option<OutputOptions>`    | unset               | `OutputOptions { file: Option<String> }`.     |
| `source_map`  | `Option<SourceMapOptions>` | off                 | Input and output source-map requests.         |

Without the transform plugin, `compile` parses and regenerates the source but does not lower TypeScript, modern syntax, or JSX.

## `parse`

```rust
pub async fn parse(
    code: String,
    options: ParseOptions,
    plugins: Vec<SharedPluginable>,
) -> Result<AstResult, CompileError>
```

### `ParseOptions`

| Field              | Type                     | Default    | Description               |
| ------------------ | ------------------------ | ---------- | ------------------------- |
| `file`             | `Option<String>`         | `index.ts` | Input file.               |
| `language`         | `Option<Language>`       | inferred   | Grammar.                  |
| `source_type`      | `Option<SourceType>`     | inferred   | Module kind.              |
| `input_source_map` | `Option<InputSourceMap>` | off        | Input source-map request. |

`parse` returns `{ ast }` and has no output map. `input_source_map` is normalized to `source_map.input` for the options hook. A plugin-set `source_map.output` or `output` is ignored.

## `rewrite`

```rust
pub async fn rewrite(
    ast: Ast,
    options: RewriteOptions,
    plugins: Vec<SharedPluginable>,
) -> Result<AstResult, CompileError>
```

### `RewriteOptions`

| Field              | Type                     | Default                    | Description               |
| ------------------ | ------------------------ | -------------------------- | ------------------------- |
| `file`             | `Option<String>`         | `index.ts`                 | Input file.               |
| `language`         | `Option<Language>`       | AST language, then file    | Grammar.                  |
| `source_type`      | `Option<SourceType>`     | AST module kind, then file | Module kind.              |
| `input_source_map` | `Option<InputSourceMap>` | off                        | Input source-map request. |

A `source_type` other than `Unambiguous` replaces the AST module kind. `Unambiguous` requests inference. `rewrite` does not reparse and has no output map.

## `generate`

```rust
pub async fn generate(
    ast: Ast,
    options: GenerateOptions,
    plugins: Vec<SharedPluginable>,
) -> Result<CodeResult, CompileError>
```

### `GenerateOptions`

| Field        | Type                       | Default      | Description                               |
| ------------ | -------------------------- | ------------ | ----------------------------------------- |
| `file`       | `Option<String>`           | `index.ts`   | Output file hint and source-map fallback. |
| `language`   | `Option<Language>`         | AST language | Grammar recorded for plugins.             |
| `output`     | `Option<OutputOptions>`    | unset        | `OutputOptions { file: Option<String> }`. |
| `source_map` | `Option<SourceMapOptions>` | off          | Input and output source-map requests.     |

Rust `generate` has no `source_type` option. The builtin generator uses `ast.program().source_type` for the module kind. `language` is resolved and visible to plugins, but the builtin generator emits from the AST grammar. To change the grammar for generation, build an AST with the desired grammar.

`generate` does not run `transform` hooks. Use `compile_from_ast` for that.

## `compile_from_ast`

```rust
pub async fn compile_from_ast(
    ast: Ast,
    options: CompileFromAstOptions,
    plugins: Vec<SharedPluginable>,
) -> Result<CodeResult, CompileError>
```

Runs prepare, transform, finalize, and generate on an existing AST. It does not reparse.

### `CompileFromAstOptions`

| Field         | Type                       | Default                    | Description                               |
| ------------- | -------------------------- | -------------------------- | ----------------------------------------- |
| `file`        | `Option<String>`           | `index.ts`                 | Input file.                               |
| `language`    | `Option<Language>`         | AST language, then file    | Grammar.                                  |
| `source_type` | `Option<SourceType>`       | AST module kind, then file | Module kind.                              |
| `output`      | `Option<OutputOptions>`    | unset                      | `OutputOptions { file: Option<String> }`. |
| `source_map`  | `Option<SourceMapOptions>` | off                        | Input and output source-map requests.     |

## Builtin Constructors

`Parser` and `Generator` call the builtin stages directly. They do not run the plugin chain. Their options are fully resolved, so every field is required. `ParserOptions` has `cwd`, `file`, `language`, `source_type`, and `source_map`; `GeneratorOptions` has `cwd`, `file`, and `source_map`.

`Generator::generate` applies `source_map.input` but not `source_map.output`. Output delivery is a pipeline concern. Inside a `generate` hook, pass `args.options` unchanged.

## `Ast`

| Accessor            | Type                 | Description                                  |
| ------------------- | -------------------- | -------------------------------------------- |
| `ast.program()`     | `&Program<'_>`       | The parsed program.                          |
| `ast.source()`      | `&Arc<str>`          | Source text the spans anchor to.             |
| `ast.language()`    | `Language`           | `JS`, `TS`, `DTS`, `JSX`, or `TSX`.          |
| `ast.source_type()` | `SourceType`         | Module kind read from `Program.source_type`. |
| `ast.map()`         | `Option<&SourceMap>` | The **input** map, if any.                   |
| `ast.with_mut`      | closure              | Mutates the program.                         |

`Ast` clones are O(1). `with_mut` copies on write when another handle is alive.

Only a `transform` hook carries mutations forward. Return `Ok(Some(TransformOutput { ast }))` from the hook.

## Passing Plugins

Wrap each plugin in `Plugin::new_shared`: `Plugin::new_shared(TransformPlugin::with_options(options))`.

## Errors

Every entry point returns `Result<_, CompileError>`. `CompileError` carries a message and an optional source. A `PluginError` returned by a hook reaches the entry point as a `CompileError` with the same message and source.

## Common Mistakes

- forgetting `Plugin::new_shared` when building the plugin list
- forgetting a Tokio runtime around `.await`
- using camelCase field names such as `sourceType` in Rust
- expecting `GenerateOptions.language` to change the builtin generator's grammar. Build an AST with the desired grammar instead
