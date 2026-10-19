# Transform Plugin Crate (`telarel_plugin_transform`)

The transform plugin. It wraps the oxc transformer and applies TypeScript/JSX/syntax lowering, runtime-helper inlining, `inject`, and `define`. In Rust it is a regular plugin named `telarel_plugin_transform`; the JS package exposes it under the builtin name `builtin:transform`.

## Key files

| Path                           | Responsibility                                                       |
| ------------------------------ | -------------------------------------------------------------------- |
| `src/helpers/`                 | Vendored helper sources, inliner, and rewriter                       |
| `src/options/define.rs`        | Compile-time `define` entries                                        |
| `src/options/helper_loader.rs` | Helper loading mode and module name                                  |
| `src/options/inject.rs`        | Injected imports keyed by local binding                              |
| `src/options/jsx.rs`           | JSX runtime and pragma options                                       |
| `src/options/mod.rs`           | `TransformOptions`, option resolution, and the raw oxc escape hatch  |
| `src/options/target.rs`        | `TransformTarget`                                                    |
| `src/options/typescript.rs`    | TypeScript transform options                                         |
| `src/plugin.rs`                | `TransformPlugin`; cached option resolution and the transform passes |

## Contract

The plugin is named `telarel_plugin_transform` by default and declares `HookUsage::Transform` only. The JS builtin wrapper constructs the same plugin with the pipeline name `builtin:transform`; Rust-created instances keep `telarel_plugin_transform`. Internally:

- `args.ast` is an owned cheap handle. The plugin moves it into a local, mutates through `Ast::with_mut` (copying on write because the driver retains the carried AST), and returns `Some(TransformOutput)` only when the tree changed. An unchanged tree returns `None`.
- Option resolution is cached in a `OnceLock`. The cache is cwd-free; the plugin context's `cwd` is overlaid per call.
- Configuration errors are cached and replay on every call. Resolution happens before mutation, so a config error leaves the input AST untouched.

## Pass order

1. Main oxc transformer (with a fresh semantic scoping)
2. Helper inlining (when the resolved mode is inline)
3. `inject`
4. `define`

Change detection for the main transformer is by code generation before and after the pass; the other passes report their own `changed` flag.

## Helper loading

Helper modes are public options: see `docs/content/plugins/transform`. Internally:

- `Inline` maps to oxc's `Runtime` mode during the transform, then a post-pass replaces the runtime imports with the vendored helper bodies.
- `Runtime` leaves the runtime import untouched.
- Inlining resolves the full dependency closure, emits dependencies before dependents, and renames helper bindings that collide with user bindings.

## Testing

The plugin module contains behavioral tests for lowering, helpers, `inject`, `define`, caching, and error paths.

```sh
cargo test -p telarel_plugin_transform
```
