# Common Crate (`telarel_common`)

Shared primitives for the compiler: the owned AST, option types, source-map handling, the builtin parser and generator, and compile errors.

## Key files

| Path                            | Responsibility                                                                   |
| ------------------------------- | -------------------------------------------------------------------------------- |
| `src/_types/options/`           | Per-entry options structs, `ResolvableOptions`, `ResolvedOptions`                |
| `src/_types/options/source_map` | Input and output source-map modes                                                |
| `src/ast/ast.rs`                | `Ast`: owned, self-referential AST bundling its allocator, source, and input map |
| `src/ast/generator.rs`          | Builtin generator; produces code and an output map                               |
| `src/ast/parser.rs`             | Builtin parser; resolves the requested input source map during parsing           |
| `src/ast/result.rs`             | `AstResult` and `CodeResult`                                                     |
| `src/contexts/compile.rs`       | `CompileContext`                                                                 |
| `src/errors/compile.rs`         | `CompileError` and code-frame rendering                                          |
| `src/errors/plugin.rs`          | `PluginError`                                                                    |
| `src/errors/source/mod.rs`      | `ErrorSource`                                                                    |
| `src/helpers/resolve.rs`        | Option resolution helpers                                                        |
| `src/sourcemap/mode.rs`         | Applies the resolved output mode after code generation                           |
| `src/sourcemap/resolve.rs`      | Input map resolution: `Value`, `Path`, `Inline`, `External`, `Auto`              |

## AST invariants

- `Ast` shares ownership of its allocator and source through `Arc`; `program()` borrows a tree whose spans anchor to that source.
- `Ast` wraps `Arc<AstInner>`; `Clone` is O(1) and shares the arena.
- `with_mut` is the only path to the arena. It takes `&mut self` and copy-on-writes through `Arc::make_mut` when another handle is alive, so mutating a clone never mutates the original.
- `with_input_map` is a deep rebuild into a fresh allocator; it shares the source `Arc` and carries the requested input map.
- `Ast` is `Send + Sync` so plugin futures can move owned handles or borrow it across threads, but sharing is read-only by contract: never mutate `Program`'s interior-mutable `Cell` fields through a shared `&Ast`.
- `Ast::map` is the **input** map (how the source was produced upstream), never the generated output map.

## Source maps

The public modes are defined by `InputSourceMapMode` and `OutputSourceMapMode` in `crates/common/src/_types/options/source_map/` and mirrored by the JS types in `packages/telarel/src/@types/source-map.ts`. Internal invariants:

- The AST's own input map always wins. The pipeline only attaches a resolved input map when the AST carries none.
- Code-entry functions (`parse`, `compile`) resolve the input map inside the builtin parser.
- Output mode is applied uniformly after the winning `generate` hook, regardless of whether the builtin or a plugin generated the code.
- When no map was produced, the code is returned byte-identical in every mode. Never fabricate an identity map.

## Option resolution

Each entry point has its own options struct. All of them convert to `ResolvableOptions`, pass through the plugin `options` fixpoint, and then resolve to `ResolvedOptions` (defaults applied).

## Testing

Source-map tests use `tempfile` for external maps.

```sh
cargo test -p telarel_common
```
