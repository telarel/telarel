# Common Crate (`telarel_common`)

Shared primitives for the compiler: the owned AST, option types, source-map handling, the builtin parser and generator, and compile errors.

## Key files

| Path                           | Responsibility                                                                                                           |
| ------------------------------ | ------------------------------------------------------------------------------------------------------------------------ |
| `src/_types/options/`          | Per-entry options structs, `ResolvableOptions`, `ResolvedOptions`                                                        |
| `src/_types/options/sourcemap` | Input and output source-map modes                                                                                        |
| `src/ast/ast.rs`               | `Ast`: owned, self-referential AST; `AstOwner` keeps the allocator, source, semantic language/source type, and input map |
| `src/ast/generator.rs`         | Builtin generator; produces code and an output map                                                                       |
| `src/ast/parser.rs`            | Builtin parser; resolves the requested input source map during parsing                                                   |
| `src/ast/result.rs`            | `AstResult` and `CodeResult`                                                                                             |
| `src/contexts/compile.rs`      | `CompileContext`                                                                                                         |
| `src/errors/compile.rs`        | `CompileError` and code-frame rendering                                                                                  |
| `src/errors/plugin.rs`         | `PluginError`                                                                                                            |
| `src/errors/source/mod.rs`     | `ErrorSource`                                                                                                            |
| `src/helpers/path.rs`          | Centralized process-cwd and path-resolution helpers                                                                      |
| `src/helpers/resolve.rs`       | Option resolution helpers                                                                                                |
| `src/sourcemap/mode.rs`        | Applies the resolved output mode after code generation                                                                   |
| `src/sourcemap/resolve.rs`     | Input map resolution: `Value`, `Inline`, `External`, `Auto`                                                              |

## AST invariants

- `Ast` shares ownership of its allocator and source through `Arc`; `program()` borrows a tree whose spans anchor to that source.
- `Ast` wraps `Arc<AstInner>`; `Clone` is O(1) and shares the arena.
- `AstOwner` owns the allocator, source, semantic `Language`/`SourceType`, and input source map.
- `Ast::language()` derives grammar from the parsed program; `Ast::source_type()` returns the coarse `telarel_common::SourceType` resolved from the program, never the parser-input `Unambiguous` for a normal parsed AST.
- The raw oxc source type remains reachable through `ast.program().source_type`; `from_source` / `try_from_source` accept the Telarel `Language` and `SourceType`, not oxc's raw type.
- `with_mut` is the only path to the arena. It takes `&mut self` and copy-on-writes through `Arc::make_mut` when another handle is alive, so mutating a clone never mutates the original.
- `with_mut` debug-asserts that `Program::source_text` stays content-equal to `Ast::source`; span conversion, codegen source maps, and error frames depend on it. Public contract: `docs/content/apis/rust/Ast.mdx`.
- `with_input_map` is a deep rebuild into a fresh allocator; it shares the source `Arc` and carries the requested input map.
- `Ast` is `Send + Sync` so plugin futures can move owned handles or borrow it across threads, but sharing is read-only by contract: never mutate `Program`'s interior-mutable `Cell` fields through a shared `&Ast`.
- `Ast::map` is the **input** map (how the source was produced upstream), never the generated output map.

## Source maps

The public modes are defined by `InputSourceMapMode` and `OutputSourceMapMode` in `crates/common/src/_types/options/sourcemap/` and mirrored by the JS types in `packages/telarel/src/@types/sourcemap/`. Internal invariants:

- The AST's own input map always wins. The pipeline only attaches a resolved input map when the AST carries none.
- Input path overrides resolve through `source_dir` and `external_map_path_for`; public path semantics: `docs/content/apis/rust/Parser.mdx` and `docs/content/apis/javascript/createParser.mdx`.
- Code-entry functions (`parse`, `compile`) resolve the input map inside the builtin parser.
- Output options are applied uniformly after the winning `generate` hook, regardless of whether the builtin or a plugin generated the code. Public output semantics: `docs/content/apis/rust/compile.mdx#compileoptions` and `docs/content/apis/rust/generate.mdx#generateoptions`.
- When no map was produced, the code is returned byte-identical in every mode. Never fabricate an identity map.

## Option resolution

Each entry point has its own options struct. All of them convert to `ResolvableOptions`, pass through the plugin `options` fixpoint, and then resolve to `ResolvedOptions` (defaults applied).

`parse` and `rewrite` expose a flat `input_source_map` on their user options; their conversions wrap it into the shared `source_map` bag with `output: None`. Plugin `options` hooks still receive the shared bag, and an `output` they set is ignored by the parse and rewrite stages. Public stage contract: `docs/content/apis/rust/parse.mdx` and `docs/content/apis/rust/rewrite.mdx`.

All internal path joining goes through `helpers/path.rs`; resolution is lexical and never canonicalizes. Internal code keeps path options as filesystem paths and converts to strings only at a boundary.

## Testing

Source-map tests use `tempfile` for external maps.

```sh
cargo test -p telarel_common
```
