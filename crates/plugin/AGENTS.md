# Plugin Crate (`telarel_plugin`)

The plugin contract and hook engine: the `Plugin` trait, object-safe `Pluginable`, hook argument/return types, per-hook ordering, the driver, and the `options` fixpoint.

## Key files

| Path                       | Responsibility                                                                                                                                         |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `src/_types/hooks/`        | Hook argument and return types                                                                                                                         |
| `src/_types/context.rs`    | `CommonPluginContext`, `PluginContext`, `ModuleInfo`                                                                                                   |
| `src/_types/order.rs`      | `PluginHookMeta` and `PluginOrder`                                                                                                                     |
| `src/_types/sourcemap.rs`  | Plugin-facing `SourceMap`                                                                                                                              |
| `src/options/fixpoint.rs`  | The `options` fixpoint runner                                                                                                                          |
| `src/plugin/mod.rs`        | The `Plugin` trait; every hook defaults to a no-op                                                                                                     |
| `src/plugin/pluginable.rs` | Object-safe `Pluginable`, `SharedPluginable`, `HookFuture`                                                                                             |
| `src/plugin_driver/hooks/` | One module per driver-run hook (`start`, `parse`, `transform`, `prepare`, `finalize`, `generate`, `end`); `options` lives in `src/options/fixpoint.rs` |
| `src/plugin_driver/mod.rs` | Partitions plugins by `HookUsage`, sorts by per-hook meta, exposes the driver                                                                          |

## Hook engine

The public hook contract is documented in `docs/content/apis/plugin`. Internally:

- `options` is not run by the driver. It is a sequential fixpoint over the plugin list (`options_fixpoint`).
- `PluginDriver::new` partitions the other hooks by declared `HookUsage`. `start` / `end` are collected from every settled plugin; the partitioned stages only from plugins that declare them.
- Each driver-run hook implementation lives in `src/plugin_driver/hooks/`; the `options` hook is implemented by `options_fixpoint` in `src/options/fixpoint.rs`. Value stages return `Option`; observers and notify hooks ignore their return value. The core owns all state and applies the returned value.
- Plugin-hook errors are propagated unchanged, with the plugin's original message and `ErrorSource`; when every plugin defers, the builtin parse/generate error is returned unchanged.

## Ordering

Each hook has its own optional `PluginHookMeta { order }`. `sort_plugins_by_hook_meta` ranks `[pre, normal, post]`, stable within a bucket. A plugin may be `pre` for one hook and `post` for another; ranking is per hook, not per plugin. `start` and `end` also use their meta but run on every settled plugin. The public meaning is in `docs/content/apis/plugin#ordering`.

## Options fixpoint

- Runs before any `PluginDriver` exists.
- `Some(OptionsArgs)` replaces the entire args bag at struct level: omitted fields fall back to defaults, not to the previous values.
- The returned plugin list replaces the settled list wholesale, including same-length swaps, reorders, and truncations.
- Newly injected plugins get their own `options` hook run on a later pass.
- A plugin already run is never re-run (tracked by identity).
- Each pass re-ranks a copy of the current list by `options_meta` to drive execution order only; the persisted/returned list keeps append/registration order.
- `MAX_OPTIONS_DEPTH` (10) guards cycles and reports the growing chain.
- Errors are propagated unchanged.

## Transform fold

`src/plugin_driver/hooks/transform.rs` owns the carried `Ast` and hands each hook an O(1) handle clone. Keeping the fold's handle alive is what forces a mutating hook's `Ast::with_mut` to copy on write, so a `None` return cannot mutate the carried AST. Public hook semantics: [plugin docs](../../docs/content/apis/plugin/index.mdx).

## Testing

The driver and fixpoint modules contain most of the behavioral coverage.

```sh
cargo test -p telarel_plugin
```
