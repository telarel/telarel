# Plugin Crate (`telarel_plugin`)

This crate holds the plugin contract and hook engine. It covers the `Plugin` trait, the object-safe `Pluginable`, hook argument and return types, per-hook ordering, the driver, and the `options` fixpoint.

## Key files

| Path                       | Responsibility                                                                                                                                         |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `src/_types/hooks/`        | Hook argument and return types                                                                                                                         |
| `src/_types/context.rs`    | `CommonPluginContext`, `PluginContext`, `ModuleInfo`                                                                                                   |
| `src/_types/order.rs`      | `PluginHookMeta` and `PluginOrder`                                                                                                                     |
| `src/_types/sourcemap.rs`  | Plugin-facing `SourceMap`                                                                                                                              |
| `src/options/fixpoint.rs`  | The `options` fixpoint runner                                                                                                                          |
| `src/plugin/mod.rs`        | The `Plugin` trait. Every hook defaults to a no-op                                                                                                     |
| `src/plugin/pluginable.rs` | Object-safe `Pluginable`, `SharedPluginable`, `HookFuture`                                                                                             |
| `src/plugin_driver/hooks/` | One module per driver-run hook (`start`, `parse`, `transform`, `prepare`, `finalize`, `generate`, `end`): `options` lives in `src/options/fixpoint.rs` |
| `src/plugin_driver/mod.rs` | Partitions plugins by `HookUsage`, sorts by per-hook meta, exposes the driver                                                                          |

## Hook engine

The public hook contract is documented in `docs/content/apis/plugin`. Internally:

- `options` is not run by the driver. It is a sequential fixpoint over the plugin list (`options_fixpoint`).
- `PluginDriver::new` partitions the other hooks by declared `HookUsage`. It collects `start` and `end` from every settled plugin. The partitioned stages run only on plugins that declare them.
- Each driver-run hook implementation lives in `src/plugin_driver/hooks/`. The `options` hook is implemented by `options_fixpoint` in `src/options/fixpoint.rs`. Value stages return `Option`. Observers and notify hooks ignore their return value. The core owns all state and applies the returned value.
- Plugin-hook errors propagate unchanged, with the plugin's original message and `ErrorSource`. When every plugin defers, the builtin parse or generate error returns unchanged.

## Ordering

Each hook has its own optional `PluginHookMeta { order }`. `sort_plugins_by_hook_meta` ranks `[pre, normal, post]` and is stable within a bucket. A plugin can be `pre` for one hook and `post` for another. Ranking is per hook, not per plugin. `start` and `end` also use their meta, but they run on every settled plugin. The public contract is in `docs/content/apis/plugin#ordering`.

## Options fixpoint

- The fixpoint runs before any `PluginDriver` exists.
- `Some(OptionsArgs)` replaces the entire args bag at struct level. Omitted fields fall back to defaults, not to the previous values.
- The returned plugin list replaces the settled list wholesale, including same-length swaps, reorders, and truncations.
- Newly injected plugins get their own `options` hook run on a later pass.
- Identity is the plugin instance (`Arc::ptr_eq` on `SharedPluginable`), not the name. See the [plugin hook semantics](../../docs/content/apis/plugin/index.mdx#hook-semantics) for the public consequences.
- Each pass re-ranks a copy of the current list by `options_meta` to drive execution order only. The persisted and returned list keeps append/registration order.
- `MAX_OPTIONS_DEPTH` (10) guards cycles and reports the growing chain.
- Errors propagate unchanged.

## Transform fold

`src/plugin_driver/hooks/transform.rs` owns the carried `Ast` and hands each hook an O(1) handle clone. The live fold handle forces a mutating hook's `Ast::with_mut` to copy on write. Thus a `None` return cannot mutate the carried AST. Public hook semantics: [plugin docs](../../docs/content/apis/plugin/index.mdx).

## Dependency boundary

Production code must not import `oxc` or `oxc_sourcemap` directly. Use the re-exports and types from `telarel_common`. Direct access belongs in `#[cfg(test)]` modules, and both crates stay dev-dependencies. Oxc is an implementation detail behind `telarel_common`. This boundary keeps the hook contract on Telarel's semantic types rather than a specific backend.

## Testing

The driver and fixpoint modules contain most of the behavioral coverage.

```sh
cargo test -p telarel_plugin
```
