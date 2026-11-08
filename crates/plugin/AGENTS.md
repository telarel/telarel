# Plugin Crate (`telarel_plugin`)

This crate holds the plugin contract and hook engine. It covers the `Plugin` trait, the object-safe `Pluginable`, hook argument and return types, per-hook ordering, the driver, and the `options` fixpoint.

## Key files

| Path                       | Responsibility                                                                                                                                         |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `src/_types/hooks/`        | Hook argument and return types                                                                                                                         |
| `src/_types/context.rs`    | `CommonPluginContext`, `PluginContext`, `ModuleInfo`                                                                                                   |
| `src/_types/order.rs`      | `PluginHookMeta` and `PluginOrder`                                                                                                                     |
| `src/options/fixpoint.rs`  | The `options` fixpoint runner                                                                                                                          |
| `src/plugin/mod.rs`        | The `Plugin` trait. Every hook defaults to a no-op                                                                                                     |
| `src/plugin/pluginable.rs` | Object-safe `Pluginable`, `SharedPluginable`, `HookFuture`                                                                                             |
| `src/plugin_driver/hooks/` | One module per driver-run hook (`start`, `parse`, `transform`, `prepare`, `finalize`, `generate`, `end`): `options` lives in `src/options/fixpoint.rs` |
| `src/plugin_driver/mod.rs` | Partitions plugins by `HookUsage`, sorts by per-hook meta, exposes the driver                                                                          |

## Hook engine

The public hook contract is documented in `docs/content/apis/plugin`. Internally:

- `options` is not run by the driver. It is a sequential fixpoint over the plugin list (`options_fixpoint`)
- `PluginDriver::new` partitions the other hooks by declared `HookUsage`. It collects `start` and `end` from every settled plugin. The partitioned stages run only on plugins that declare them
- each driver-run hook implementation lives in `src/plugin_driver/hooks/`. The `options` hook is implemented by `options_fixpoint` in `src/options/fixpoint.rs`. Value stages return `Option`. Observers and notify hooks ignore their return value. The core owns all state and applies the returned value
- for AST entries, `ModuleInfo.source_type` uses an explicit module kind other than `Unambiguous`. When the option is `Unambiguous` or absent, the AST module kind applies. If the AST has no module kind, the file extension is the fallback
- plugin-hook errors propagate unchanged, with the plugin's original message and `ErrorSource`. When every plugin defers, the builtin parse or generate error returns unchanged

## Ordering

Each hook has its own optional `PluginHookMeta { order }`. `sort_plugins_by_hook_meta` ranks `[pre, normal, post]` and is stable within a bucket. A plugin can be `pre` for one hook and `post` for another. Ranking is per hook, not per plugin. `start` and `end` also use their meta, but they run on every settled plugin. The public contract is in `docs/content/apis/plugin#ordering`.

## Options fixpoint

- the fixpoint runs before any `PluginDriver` exists
- `Some(OptionsArgs)` replaces the entire args bag at struct level. Omitted fields fall back to defaults, not to the previous values
- the returned plugin list replaces the settled list wholesale, including same-length swaps, reorders, and truncations
- newly injected plugins get their own `options` hook run on a later pass
- identity is the plugin instance (`Arc::ptr_eq` on `SharedPluginable`), not the name. See the [plugin hook semantics](../../docs/content/apis/plugin/index.mdx#hook-semantics) for the public consequences
- each pass re-ranks a copy of the current list by `options_meta` to drive execution order only. The persisted and returned list keeps append/registration order
- `MAX_OPTIONS_DEPTH` (10) guards cycles and reports the growing chain
- errors propagate unchanged

## Transform fold

`src/plugin_driver/hooks/transform.rs` owns the carried `Ast` and hands each hook an O(1) handle clone. The live fold handle forces a mutating hook's `Ast::with_mut` to copy on write. Thus a `None` return cannot mutate the carried AST. Public hook semantics: [plugin docs](../../docs/content/apis/plugin/index.mdx).

The fold partitions the settled transform list into maximal contiguous runs whose plugins return `Some` from `Pluginable::as_transform_batchable`. The first member of a run builds one `TransformBatch` through `TransformBatchable::make_transform_batch`. A batch returns the same `Option<TransformOutput>` shape as a single hook. A run of one, or a batch construction that returns `None`, uses the single-hook path. A non-batchable plugin breaks the run around it. The batch traits are internal capabilities only. They are exported through `crate::__internal` and are not public plugin API.

A batch that reports `None` leaves the carried AST untouched. The batch read-back compares its final output with its input. A batch whose members changed and then reverted the AST still reports `None` when no comments need re-anchoring. An earlier accepted plugin or batch output remains carried, and the final result stays `Some`. When comments need re-anchoring, the batch reports `Some` with the sanitized input AST instead. This behavior is required by the batch read-back. The batch read-back differs from a single hook chain, where an accepting hook returns `Some`.

## Dependency boundary

Production code must not import `oxc` or `oxc_sourcemap` directly. Use the re-exports and types from `telarel_common`. Direct access belongs in `#[cfg(test)]` modules, and both crates stay dev-dependencies. Oxc is an implementation detail behind `telarel_common`. This boundary keeps the hook contract on Telarel's semantic types rather than a specific backend.

## Testing

The driver and fixpoint modules contain most of the behavioral coverage.

```sh
cargo test -p telarel_plugin
```
