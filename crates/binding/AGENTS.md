# Binding Crate (`telarel_binding`)

The NAPI binding between the Rust compiler and the `telarel` JavaScript package. It owns the JS-facing functions, the option and AST marshalling, the JS plugin bridge, builtin plugin dispatch, and the worker task model.

## Key files

| Path                       | Responsibility                                                         |
| -------------------------- | ---------------------------------------------------------------------- |
| `src/_types/`              | JS-facing options, results, hooks, source maps, AST shapes             |
| `src/ast/`                 | Crate-private AST normalization and ESTree JSON codec                  |
| `src/comment/`             | Comment normalization and injection layer                              |
| `src/errors/`              | Private error envelopes and their shared source payload                |
| `src/functions/`           | Seven NAPI entry points                                                |
| `src/plugin/builtin/`      | Rust builtin plugin dispatch and transform option parsing              |
| `src/plugin/build.rs`      | `bridge_plugin` / `to_plugins`, builtin dispatch by `__builtin`        |
| `src/plugin/hooks.rs`      | TSFN payloads, `RefList`, `SharedRef`, `SharedStr`                     |
| `src/plugin/pluginable.rs` | `JsPlugin`: hook scanning, TSFN calls, marshalling                     |
| `src/plugin/spans.rs`      | UTF-8 to UTF-16 span conversion with an ASCII identity fast path       |
| `src/tasks/function.rs`    | `FunctionTask`, the worker closure, and reference release in `finally` |
| `src/tasks/mod.rs`         | Tokio current-thread runtime per thread and `block_on_compile`         |

## JS API surface

`docs/content/apis/javascript` documents the public JavaScript surface. The seven NAPI functions live in `src/functions/`. The source (text or AST) is always the first argument. Options never carry `code`. `builtinParse` and `builtinGenerate` bypass the plugin chain. `builtinGenerate` resolves `sourceMap.input` through `Generator` and applies `sourceMap.output` delivery itself.

`parse` and `rewrite` take a flat `inputSourceMap`. `compile`, `compileFromAst`, and `generate` keep the shared `sourceMap` bag plus `output`; `builtinParse` and `builtinGenerate` take only `sourceMap`. The `options` payload and the resolved `start` options carry both. The `parse` hook option payload carries only `sourceMap`. The `generate` hook payload carries `sourceMap` with both `input` and `output` unset, serialized as `{ input: null, output: null }`. AST and transform payloads carry neither.

`cwd` stays a JS string at the napi boundary. The JS-facing option structs keep `String`, and both `_types/options.rs` and `unmarshal_options` convert it to `PathBuf` on entry. `_types/path.rs::path_label` renders it back at every outbound site. Non-UTF-8 paths become `U+FFFD`. `file` stays a UTF-8 string (`String`) in both directions. Because napi rejects non-UTF-8 paths on Unix, do not expose `PathBuf` in outbound napi structs.

## AST boundary

- `src/ast/` is crate-private. Its `normalized::JsAst` and `normalized::JsAstInput` mirrors stay inside the crate. The crate surface exposes only `_types::ast::{JsAst, JsAstInput, JsAstResult}`, re-exported from `lib.rs`.
- `rewrite` and `compileFromAst` use the caller's `sourceType` when it is not `"unambiguous"`. When the option is `"unambiguous"` or absent, the module kind comes from `program.sourceType`. If the program has no `sourceType`, the file extension is the fallback. `generate` takes the module kind from `program.sourceType` unless an options hook replaces it.
- The `parse` and `generate` hooks use the normalized object `{ program, code, map?, language? }`. Outbound `JsAst.language` is required. Inbound `JsAstInput.language` is optional.
- The `prepare`, `transform`, and `finalize` hooks use a JSON-serialized ESTree string (`astJson`) through `oxc_estree_codec`. The payload also carries the AST's `code`, optional `map`, and language label. The JS wrapper rebuilds the normalized `Ast` from those fields. Transform read-back uses the returned language when present, otherwise the input AST's language. `program.sourceType` owns the module kind.
- A `parse` hook replacement keeps the grammar authority of the parser options. The returned AST's language does not override them.
- Before it builds the UTF-16 `SpanMap`, outbound ESTree serialization debug-asserts that `Program::source_text` is still content-equal to `Ast::source`. ASCII sources use identity offsets and skip the table and JSON rewrite; non-ASCII sources still build and rewrite through the table.
- The bridge compares the JS `transform` return in its UTF-16 JSON form plus `code`, `map`, and language. An unchanged AST short-circuits to `None` unless its comments need re-anchoring, in which case the sanitized AST returns as `Some`. Comments live inside `programJson` and flow through `ast_from_json`, and rebuilt programs anchor to the returned `code` and `map`. A returned transform AST owns its comments. `comments: []` or an absent key drops them.
- A batched transform run uses the same read-back helper as a single transform. The batch payload carries the run's raw descriptors; the JS batch runner returns one final output, and the binding applies the same equality, comment re-anchor, language fallback, and rebuild rules. Do not duplicate the comparison logic.
- A batch whose final output equals its input reads back as `None` when no comments need re-anchoring, even when an earlier batch member changed the AST. When comments need re-anchoring, the sanitized input AST returns as `Some`. The driver keeps any earlier accepted output, so the final result can still be `Some`. This differs from a single hook chain, where the accepting hook returns `Some`. The compare-with-input read-back is required; do not widen it.
- No `Ast` or pipeline future crosses the JS and worker boundary. The worker closure produces the JS-facing value directly.

## Comment layer

`src/comment/` normalizes inbound comments and injects outbound ones at the AST boundary. [Ast](/apis/javascript/Ast) documents the public contract.

- The adapters are `CANONICAL` and `START_END`, folded with the first `Some` value per field. `CANONICAL` owns `attachedTo`, `position`, `newlines`, and `content`. It makes sure that the `position` and `content` labels are valid. `START_END` supplies the `start`/`end` pair. Normalization overwrites the requested position with leading.
- `inject` splices the top-level `comments` array into the outbound program JSON with UTF-16 spans as its first key. It returns the JSON unchanged when there are no comments.
- `extract_raw` scans inbound program JSON for the top-level `comments` value and deserializes only that array. It runs before span conversion because comment spans are UTF-16. The literal prefilter keeps the no-comments path cheap. Any `\u` escape also triggers the scan, so the scan recognizes an escaped key.
- `normalize_from_program` derives anchors from the rebuilt program's printable starts and converts those starts with `SpanMap`. It then normalizes the entries. `attach_with_starts` allocates the normalized comments into the rebuilt program's arena and re-anchors them to those same starts.
- The re-anchor starts are the program start and end, directives, and non-block, non-empty statements. The same attach step also demotes pure and no-side-effects annotations to normal comments.
- The transform pass-through re-anchors the existing AST in place. It leaves pure and no-side-effects annotations to the oxc annotation stash.
- Normalization errors identify the comment index and, when known, its offsets. Deserialization errors identify only the index. A non-array top-level `comments` value names the field.

## JS plugins

`docs/content/apis/plugin/javascript` documents the public plugin API. Internally:

- `JsPlugin::from_object` makes sure that `name` is present and scans the hook slots into TSFNs. `read_hook_meta` reads the `<hook>Meta` siblings that the JS wrapper produces.
- The bridge infers hook usage from the TSFNs that the adapter carries.
- A plugin is transform-batchable only when it has both the transform TSFN and the `__telarelTransformBatch` TSFN. The wrapper attaches that runner plus `__telarelTransformEntry` (the user handler) as non-enumerable properties on each transform descriptor. The binding scan uses `TRANSFORM_BATCH_KEY`, which must match `TRANSFORM_BATCH` in the wrapper.
- `JsPlugin` keeps a clone of the caller's rooted descriptor. `make_transform_batch` downcasts the run members to `JsPlugin` and sends their descriptor references in driver order. It never creates a second root; `to_plugins` owns the release list.
- The JS batch runner isolates members with a checkpoint. A member that mutates and returns `null` is discarded by reparsing the checkpoint; an accepted change refreshes it. The runner also clears the sticky mutation marker before each member and normalizes an absent returned language to the carried AST's language.
- The checkpoint stores the input `map` by reference. Direct in-place mutation of `ast.map` outside `ast.mutate` is not isolated between batch members; the single path gives each plugin a fresh map. `ast.mutate` is the supported mutation path. Do not clone maps per member, because that erodes the batching performance win.
- Batching is default-on. `TELAREL_DISABLE_TRANSFORM_BATCH=1` is an internal test and diagnostic opt-out, not public package API; it forces the per-plugin path. `TELAREL_DEBUG_TRANSFORM_BATCH=1` enables the internal batch trace, which is silent by default.
- The wrapper assigns each original plugin object an internal id through a `WeakMap`. It attaches the id to the generated raw descriptor as a non-enumerable `__telarelPluginId` property. The id is internal and is not serialized. A spread does not copy it.
- `to_plugins` keeps a per-compile `RefList` memo from id to adapter. It reuses the adapter when the same id appears again, so an unchanged plugin's `options` hook runs once. `RefList::release` clears the memo to break the `RefList -> JsPlugin -> RefList` cycle, because `JsPlugin` holds the `RefList`.
- If code mutates a plugin object after its first bridge, the plugin keeps the adapter from that first bridge. Identity is the instance.
- The `options` hook is special. It is an async function that returns a promise. Telarel bridges its resolution on the JS thread because the bridge must root and wrap late plugin descriptors where JS objects are reachable.
- The pre-resolution `options` payload omits unset `cwd` and `file`. Absent means unset. Resolved hook payloads always carry both. Do not reintroduce empty-string sentinels.
- Rooted descriptors live on the compile's `RefList`. `FunctionTask::finally` drains them. Do not leak or double-release them.
- Hook failures decode the private `PluginError` envelope from the message. Ordinary `napi::Error` values keep their existing rendering.

## Error transport

- Compile rejections and plugin hook failures share the optional source payload `{ source: { file, code, span } }`.
- Spans are UTF-16 code units on the JS side. Rust converts them against the payload's own `code` before it constructs a byte-offset `Span`.
- Telarel exact-matches envelope markers. Unknown or malformed envelopes produce a plain message error.

## Builtin plugins

- Telarel dispatches a plugin to Rust only when it carries the explicit `__builtin: true` property. The binding reserves no name prefix.
- The descriptor is plain data: `{ __builtin: true, name, options }`.
- Telarel names the constructed Rust plugin after the builtin's JS name (for transform, `builtin:transform`), so pipeline errors use the JS-facing name. Identity is the adapter instance, not the name.
- Builtins use the same `__telarelPluginId` and `RefList` adapter memo path as JS hook plugins.
- The same dispatch applies mid-fixpoint to plugins that `options` injects.

## Execution model

- Every NAPI entry point runs its whole call on a libuv worker thread through `FunctionTask`.
- `block_on_compile` reuses a Tokio current-thread runtime for each thread. NAPI tasks run one at a time per worker, so concurrent `block_on` on that runtime is not possible.
- Hook futures are `Send`. `prepare`, `finalize`, and `generate` borrow the AST read-only. `transform` receives an owned cheap `Ast` handle.

## Testing

The JS integration suite lives in `tests/telarel` and runs against both native and WASI backends.

Run `just test-js-native` for the native suite. `just test-js` runs both native and WASI.

```sh
cargo test -p telarel_binding
```
