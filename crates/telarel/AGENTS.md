# Telarel Crate (`telarel`)

The public Rust API surface. It re-exports the pipeline, plugin system, and shared types, and exposes feature-gated oxc modules. It contains no compiler behavior.

## Key files

| Path         | Responsibility                                  |
| ------------ | ----------------------------------------------- |
| `src/lib.rs` | Public re-exports and feature-gated oxc modules |

## Re-export rules

- Add public API items as re-exports here; do not add wrapper logic.
- Keep each re-export tied to its owning crate: `telarel_common`, `telarel_plugin`, or `telarel_core`.
- `ast_visit`, `semantic`, and `traverse` are feature-gated; `traverse` also enables `ast_visit` and `semantic`.
- `allocator`, `span`, `str`, `syntax`, `ast`, and `sourcemap` are always exposed.

## Testing

There are no crate-local tests. Consumer examples live in `examples/rust`; JavaScript integration tests live in `tests/telarel`. Verify re-export changes with `cargo check -p telarel`.
