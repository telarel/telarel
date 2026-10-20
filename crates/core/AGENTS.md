# Core Crate (`telarel_core`)

The pipeline orchestrator. It exposes five entry points over one shared skeleton.

## Key files

| Path                                | Responsibility                                           |
| ----------------------------------- | -------------------------------------------------------- |
| `src/common/pipeline.rs`            | `Stage`, `run`, and the stage plan for every entry point |
| `src/common/test_util.rs`           | Shared test plugins and fixtures                         |
| `src/functions/compile.rs`          | `compile(code, options, plugins)`                        |
| `src/functions/compile_from_ast.rs` | `compile_from_ast(ast, options, plugins)`                |
| `src/functions/generate.rs`         | `generate(ast, options, plugins)`                        |
| `src/functions/parse.rs`            | `parse(code, options, plugins)`                          |
| `src/functions/rewrite.rs`          | `rewrite(ast, options, plugins)`                         |

The `functions/*` modules are thin: convert options to `ResolvableOptions`, select the stage, call `run`. Put behavior in `pipeline.rs` unless it is truly entry-point-specific.

## Pipeline order

Every invocation runs:

1. `options` fixpoint (before any driver exists)
2. build `PluginDriver` from the settled plugin list
3. resolve options and build the `PluginContext`
4. `start`
5. the entry point's stages, inline
6. `end`

`options`, `start`, and `end` fire exactly once per invocation. No entry point re-enters another.

`end` runs on every path after the options fixpoint settles, including stage and `start` failures. The only path that skips `end` is an options-fixpoint failure, because no driver exists yet. On a stage failure, a failing `end` returns its own error instead of the stage error; on a success path it is the error.

## Stage plans

Which hooks each entry point runs is public behavior: see `docs/content/apis/plugin#entry-points`. Internally each entry point is a `Stage` implementation in `pipeline.rs`; `run` never re-enters another entry point.

## Source maps

Shared input/output invariants: [common source maps](../common/AGENTS.md#source-maps).

## Error propagation

- Hook errors cross the pipeline unchanged; the pipeline adds no stage prefixes, so the message and `ErrorSource` are exactly what the plugin or builtin produced.
- The original stage error is passed to `end` as-is.
- A failing `end` returns its own error instead of the stage error; a succeeding `end` leaves the stage error untouched.

## Testing

Pipeline behavior is covered by unit tests in each `functions/*` module and in `pipeline.rs`. JavaScript integration tests live in `tests/telarel`.

```sh
cargo test -p telarel_core
```
