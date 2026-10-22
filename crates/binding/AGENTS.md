# Binding Crate (`telarel_binding`)

The NAPI binding between the Rust compiler and the `telarel` JavaScript package. It owns the JS-facing functions, AST/option marshalling, the JS plugin bridge, builtin-plugin dispatch, and the worker-task model.

## Key files

| Path                       | Responsibility                                                        |
| -------------------------- | --------------------------------------------------------------------- |
| `src/_types/`              | JS-facing options, results, hooks, source maps, AST shapes            |
| `src/errors/`              | Private error envelopes and their shared source payload               |
| `src/functions/`           | Seven NAPI entry points                                               |
| `src/plugin/builtin/`      | Rust builtin plugin dispatch and transform option parsing             |
| `src/plugin/build.rs`      | `bridge_plugin` / `to_plugins`; builtin dispatch by `__builtin`       |
| `src/plugin/hooks.rs`      | TSFN payloads, `RefList`, `SharedRef`, `SharedStr`                    |
| `src/plugin/pluginable.rs` | `JsPlugin`: hook scanning, TSFN calls, marshalling                    |
| `src/tasks/function.rs`    | `FunctionTask`; the worker closure and reference release in `finally` |
| `src/tasks/mod.rs`         | Thread-local current-thread Tokio runtime and `block_on_compile`      |
| `src/ast.rs`               | AST normalization and ESTree JSON codec                               |

## JS API surface

The public JavaScript surface is documented in `docs/content/apis/javascript`. Internally, the seven NAPI functions live in `src/functions/`: the source (text or AST) is always the first argument, options never carry `code`, and `builtinParse` / `builtinGenerate` bypass the plugin chain.

`parse` and `rewrite` take a flat `inputSourceMap`; every other entry point keeps the shared `sourceMap` bag, and every hook payload uses it.

`cwd` stays a JS string at the napi boundary: the JS-facing option structs keep `String`, and both `_types/options.rs` and `unmarshal_options` convert it to `PathBuf` on entry. `_types/path.rs::path_label` renders it back at every outbound site; non-UTF-8 paths become `U+FFFD`. `file` stays a UTF-8 string (`String`) in both directions. Do not expose `PathBuf` in outbound napi structs: napi rejects non-UTF-8 paths on Unix.

## AST boundary

- `parse` / `generate` hooks use the normalized object `{ program, code, map? }`.
- `prepare` / `transform` / `finalize` use a JSON-serialized ESTree string (`astJson`) via `oxc_estree_codec`, plus the AST's `code` and optional `map`. The JS wrapper rebuilds the normalized `Ast` from those fields.
- Outbound ESTree serialization debug-asserts that `Program::source_text` is still content-equal to `Ast::source` before building the UTF-16 `SpanMap`.
- The JS `transform` return is compared in its UTF-16 JSON form plus `code` and `map`; an unchanged AST short-circuits to `None`. Rebuilt programs anchor to the returned `code`/`map` and reattach the original comments, because the ESTree codec skips `Program.comments`.
- No `Ast` or pipeline future crosses the JS/worker boundary. The worker closure produces the JS-facing value directly.

## JS plugins

The public plugin API is documented in `docs/content/apis/plugin/javascript`. Internally:

- `JsPlugin::from_object` validates `name` and scans the hook slots into TSFNs; `read_hook_meta` reads the `<hook>Meta` siblings produced by the JS wrapper.
- Hook usage is inferred from the TSFNs present on the adapter.
- The `options` hook is special: it is an async function returning a promise. Its resolution is bridged on the JS thread, because late plugin descriptors must be rooted and wrapped where JS objects are reachable.
- Rooted descriptors live on the compile's `RefList` and are drained in `FunctionTask::finally`. Do not leak or double-release them.
- Hook failures decode the private `PluginError` envelope from the message; ordinary `napi::Error` values keep their existing rendering.

## Error transport

- Compile rejections and plugin hook failures share the optional source payload `{ source: { file, code, span } }`.
- Spans are UTF-16 code units on the JS side; Rust converts them against the payload's own `code` before constructing a byte-offset `Span`.
- Envelope markers are exact-matched; unknown or malformed envelopes fall back to a plain message error.

## Builtin plugins

- A plugin is dispatched to Rust only when it carries the explicit `__builtin: true` property. No name prefix is reserved.
- The descriptor is plain data: `{ __builtin: true, name, options }`.
- The constructed Rust plugin is renamed to the builtin's JS name (for transform, `builtin:transform`) so pipeline errors and identity use the JS-facing name.
- The same dispatch applies mid-fixpoint for plugins injected by `options`.

## Execution model

- Every NAPI entry point runs its whole call on a libuv worker thread through `FunctionTask`.
- `block_on_compile` reuses a per-thread current-thread Tokio runtime. NAPI tasks run one at a time per worker, so concurrent `block_on` on that runtime is not possible.
- Hook futures are `Send`. `prepare` / `finalize` / `generate` borrow the AST read-only; `transform` receives an owned cheap `Ast` handle.

## Testing

The JS integration suite lives in `tests/telarel` and runs against both native and WASI backends.

Run `just test-js-native` for the native suite; `just test-js` runs both native and WASI.
