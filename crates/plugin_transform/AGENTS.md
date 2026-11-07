# Transform Plugin Crate (`telarel_plugin_transform`)

The transform plugin. It wraps the oxc transformer. It applies TypeScript and syntax lowering. It lowers JSX only when configured. It also applies runtime-helper inlining, `inject`, and `define`. In Rust it is a regular plugin named `telarel_plugin_transform`. The JS package exposes it under the builtin name `builtin:transform`.

## Key files

| Path                           | Responsibility                                                      |
| ------------------------------ | ------------------------------------------------------------------- |
| `src/helpers/`                 | Vendored helper sources, inliner, and rewriter                      |
| `src/options/define.rs`        | Compile-time `define` entries                                       |
| `src/options/helper_loader.rs` | Helper loading mode and module name                                 |
| `src/options/inject.rs`        | Injected imports keyed by local binding                             |
| `src/options/jsx.rs`           | JSX runtime and pragma options                                      |
| `src/options/mod.rs`           | `TransformOptions`, option resolution, and the raw oxc escape hatch |
| `src/options/output.rs`        | Output-file derivation from the input file                          |
| `src/options/target.rs`        | `TransformTarget`                                                   |
| `src/options/typescript.rs`    | TypeScript transform options                                        |
| `src/plugin.rs`                | `TransformPlugin`, output derivation, cached options, and passes    |

## Contract

The plugin is named `telarel_plugin_transform` by default. It declares `HookUsage::Transform` only. The `options` hook runs unconditionally and is not gated by the declared usage. The JS builtin wrapper constructs the same plugin with the pipeline name `builtin:transform`. Rust-created instances keep `telarel_plugin_transform`.

Internally:

- `args.ast` is an owned cheap handle. The plugin moves it into a local and mutates through `Ast::with_mut`. `with_mut` copies on write because the driver retains the carried AST. `None` leaves the carried AST untouched.
- Change detection is per pass: the main pass compares code generation before and after the oxc transformer runs, helper inlining returns whether it edited the program, and `inject` and `define` report their own `changed` flag. The plugin returns `Some(TransformOutput)` when any pass reports a change.
- Option resolution is cached in a `OnceLock`. The cache is cwd-free. The plugin context's `cwd` is overlaid per call.
- Configuration errors are cached and replay on every call. Resolution happens before mutation, so a configuration error leaves the input AST untouched.

## Options hook

`options_meta` returns `Post`, so the `options` hook sees the `file` and `language` settled by the normal hooks. It derives `output.file` when the caller left it unset or empty, then returns the updated bag.

A later `Post` options hook that changes `file` or `language` must set `output.file` itself. The plugin does not derive the name again for that instance after a later hook mutates the bag.

The plugin cannot see the AST language when it derives `output.file`. In `compileFromAst`, a caller whose AST language differs from the grammar implied by `file` and `language` must set `output.file` explicitly. `rewrite` has the same limitation but produces no output.

Without this plugin in the chain, nothing derives an output name. The resolved `output.file` stays `file`.

## TypeScript

The curated option names follow `tsconfig`. `overlay_typescript` writes into three oxc layers: `typescript`, `decorator`, and `assumptions`.

- `verbatimModuleSyntax` maps to `typescript.only_remove_type_imports`.
- `rewriteRelativeImportExtensions` maps to `typescript.rewrite_import_extensions`.
- `experimentalDecorators` maps to `decorator.legacy`. The binding cannot set that oxc field directly because oxc marks it `#[serde(skip)]`.
- `emitDecoratorMetadata` maps to `decorator.emit_decorator_metadata`. The plugin validates that the resolved legacy flag is true when the curated option is true.
- `useDefineForClassFields` sets `assumptions.set_public_class_fields` and `typescript.remove_class_fields_without_initializer` to the inverse of the option, and sets `typescript.allow_declare_fields` to `true`. Public default and precedence: `docs/content/plugins/transform.mdx#typescript`.
- `optimizeConstEnums` maps to `typescript.optimize_const_enums`.

`allowNamespaces` and `optimizeEnums` are raw oxc fields only.

Classic JSX pragmas are copied from `jsx.pragma` and `jsx.pragma_frag` into `typescript.jsx_pragma` and `typescript.jsx_pragma_frag`. Public runtime behavior: `docs/content/plugins/transform.mdx#typescript`.

## JSX

JSX lowering follows the public contract at `docs/content/plugins/transform.mdx#jsx`. Internally, `Preserve` clears the oxc JSX pass flags.

## Output file

`TransformOptions::output_file` applies the mapping in `options/output.rs`. Public contract: `docs/content/plugins/transform.mdx#output-file`.

`telarel_common::FILE_DEFAULT` names the fallback input and `strip_file_suffix` removes query and hash suffixes.

## Pass order

1. Main oxc transformer (with a fresh semantic scoping)
2. Helper inlining (when the resolved mode is inline)
3. `inject`
4. `define`

## Helper loading

The public API exposes the helper modes: see `docs/content/plugins/transform`. Internally:

- `Inline` maps to oxc's `Runtime` mode during the transform, then a post-pass replaces the runtime imports with the vendored helper bodies.
- `Runtime` leaves the runtime import untouched.
- Inlining resolves the full dependency closure, emits dependencies before dependents, and renames helper bindings that collide with user bindings.

## Testing

The plugin module contains behavioral tests for lowering, helpers, `inject`, `define`, caching, and error paths.

```sh
cargo test -p telarel_plugin_transform
```
