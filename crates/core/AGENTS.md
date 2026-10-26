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

The `functions/*` modules are thin. They convert options to `ResolvableOptions`, select the stage, and call `run`. Put behavior in `pipeline.rs` unless it is truly entry-point-specific.

## Pipeline order

Every invocation runs:

1. `options` fixpoint (before any driver exists)
2. build `PluginDriver` from the settled plugin list
3. resolve options and build the `PluginContext`
4. `start`
5. the entry point's stages, inline
6. `end`

AST-entry stages resolve language in this order: explicit options, then AST language. They resolve the module kind in this order: explicit `source_type`, then the AST `source_type`, then the file extension. A concrete explicit kind replaces `Program.source_type` before the stages run. `ModuleInfo` and post-resolution hooks see the resolved values.

Each stage runs once per invocation. `options` runs per plugin instance. `start` and `end` run per settled entry, duplicates included. No entry point re-enters another.

`end` runs on every path after the options fixpoint settles. This includes stage failures and `start` failures. The only path that skips `end` is an options-fixpoint failure, because no driver exists yet. On a stage failure, a failing `end` returns its own error instead of the stage error. On the success path, the `end` error is the returned error.

## Stage plans

Which hooks each entry point runs is public behavior. See `docs/content/apis/plugin#entry-points`. Internally each entry point is a `Stage` implementation in `pipeline.rs`. `run` never re-enters another entry point.

## Source maps

Shared input/output invariants: [common source maps](../common/AGENTS.md#source-maps).

## Error propagation

- Hook errors cross the pipeline unchanged. The pipeline adds no stage prefixes. The message and `ErrorSource` are exactly what the plugin or builtin produced.
- The original stage error is passed to `end` as-is.
- A failing `end` returns its own error instead of the stage error. A succeeding `end` leaves the stage error untouched.

## Dependency boundary

Production code must not import `oxc` or `oxc_sourcemap` directly. Use the re-exports and types from `telarel_common`. Direct access belongs in `#[cfg(test)]` modules, and both crates stay dev-dependencies. Oxc is an implementation detail behind `telarel_common`. This boundary keeps the pipeline on Telarel's semantic types rather than a specific backend.

## Testing

Pipeline behavior is covered by unit tests in each `functions/*` module and in `pipeline.rs`. JavaScript integration tests live in `tests/telarel`.

```sh
cargo test -p telarel_core
```
