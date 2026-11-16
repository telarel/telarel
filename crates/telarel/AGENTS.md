# Telarel Crate (`telarel`)

The crate is the public Rust API surface. It re-exports the pipeline, the plugin system, and shared types. It also exposes oxc modules, some feature-gated. It contains no compiler behavior.

## Key files

| Path         | Responsibility                    |
| ------------ | --------------------------------- |
| `src/lib.rs` | Public re-exports and oxc modules |

## Re-export rules

- add public API items as re-exports here. Do not add wrapper logic
- keep each re-export tied to its owning crate: `telarel_common`, `telarel_plugin`, or `telarel_core`
- re-export `ParserOptions`, `GeneratorOptions`, `IndentStyle`, `PartialOxcParseOptions`, `PartialOxcCodegenOptions`, `PartialOxcCommentOptions`, the raw `Oxc*` aliases, and the resolved builtin and stage types from `telarel_common`. Do not wrap them
- do not re-export `FILE_DEFAULT`, `strip_file_suffix`, or the low-level `parse_*` / `generate_*` primitives. They are shared with sibling crates and tests but are not part of the supported user-facing API. The user-facing stages are `Parser` and `Generator`
- `ast_visit`, `semantic`, and `traverse` are feature-gated. `traverse` also enables `ast_visit` and `semantic`
- `allocator`, `span`, `str`, `syntax`, `ast`, `oxc`, and `sourcemap` are always exposed
- the `oxc` module is the raw option-type path for callers: `oxc::parser::ParseOptions` and `oxc::codegen::{CodegenOptions, CommentOptions, IndentChar, LegalComment}`. It is not feature-gated. Do not duplicate those raw types in the crate

## Testing

There are no crate-local tests. Consumer examples live in `examples/rust`. JavaScript integration tests live in `tests/telarel`. To make sure that re-export changes compile, run `cargo check -p telarel`.
