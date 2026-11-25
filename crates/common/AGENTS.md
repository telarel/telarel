# Common Crate (`telarel_common`)

Shared primitives for the compiler: the owned AST, option types, source-map handling, the builtin parser and generator, and compile errors.

## Key files

| Path                           | Responsibility                                                                                                                |
| ------------------------------ | ----------------------------------------------------------------------------------------------------------------------------- |
| `src/_types/options/`          | Per-entry options structs, `ResolvableOptions`, `ResolvedOptions`, and the `From` conversion impls                            |
| `src/_types/options/builtin/`  | User-facing `ParserOptions` / `GeneratorOptions`, partial `oxc` patches, `IndentStyle`, resolved types, and stage defaults    |
| `src/_types/options/sourcemap` | Input and output source-map modes                                                                                             |
| `src/ast/ast.rs`               | `Ast` is an owned, self-referential AST. `AstOwner` keeps the allocator, source, semantic language/source type, and input map |
| `src/ast/generator.rs`         | Builtin generator. Applies `source_map.input`, then produces code and an output map that records the output file              |
| `src/ast/parser.rs`            | Builtin parser. Resolves the requested input source map during parsing                                                        |
| `src/ast/result.rs`            | `AstResult` and `CodeResult`                                                                                                  |
| `src/contexts/compile.rs`      | `CompileContext`                                                                                                              |
| `src/errors/compile.rs`        | `CompileError` and code-frame rendering                                                                                       |
| `src/errors/plugin.rs`         | `PluginError`                                                                                                                 |
| `src/errors/source/mod.rs`     | `ErrorSource`                                                                                                                 |
| `src/helpers/path.rs`          | Centralized process-cwd and path-resolution helpers                                                                           |
| `src/helpers/resolve.rs`       | Option resolution helpers                                                                                                     |
| `src/sourcemap/mode.rs`        | Applies the resolved output file and mode after code generation                                                               |
| `src/sourcemap/resolve.rs`     | Input map resolution: `Value`, `Inline`, `External`, `Auto`; `attach_input_map` applies the precedence rule                   |

## AST invariants

- `Ast` shares ownership of its allocator and source through `Arc`. `program()` borrows a tree whose spans anchor to that source
- `Ast` wraps `Arc<AstInner>`. `Clone` is O(1) and shares the arena
- `AstOwner` owns the allocator, source, semantic `Language`/`SourceType`, and input source map
- `Ast::language()` derives the grammar from the parsed program. `Ast::source_type()` returns the coarse `telarel_common::SourceType` resolved from the program. It never returns the parser-input `Unambiguous` for a normal parsed AST. An explicit `source_type` other than `Unambiguous` replaces `Program.source_type` when an AST enters the pipeline. The accessors read the program
- the raw oxc source type remains reachable through `ast.program().source_type`. `from_source` / `try_from_source` accept the Telarel `Language` and `SourceType`, not oxc's raw type
- `with_mut` is the only path to the arena. It takes `&mut self` and copy-on-writes through `Arc::make_mut` when another handle is alive, so mutating a clone never mutates the original
- `with_mut` debug-asserts that `Program::source_text` stays content-equal to `Ast::source`. Span conversion, codegen source maps, and error frames depend on that equality. Public contract: `docs/content/apis/rust/Ast.mdx`
- `with_input_map` rebuilds deeply into a fresh allocator. It shares the source `Arc` and carries the requested input map
- `Ast` is `Send + Sync`, so plugin futures can move owned handles or borrow it across threads. Callers that share `&Ast` across threads must use it read-only. Never mutate `Program`'s interior-mutable `Cell` fields through a shared `&Ast`
- `Ast::map` is the **input** map (how the source was produced upstream), never the generated output map

## Source maps

The public modes are defined by `InputSourceMapMode` and `OutputSourceMapMode` in `crates/common/src/_types/options/sourcemap/` and mirrored by the JS types in `packages/telarel/src/@types/sourcemap/`. Internal invariants:

- an explicit input request replaces the AST's own input map when it resolves. A best-effort `Inline` or `Auto` miss keeps the AST's own map, and an `External` miss or corrupt map is an error. `attach_input_map` implements this rule for the pipeline entry boundaries and the builtin generator
- input path overrides resolve through `source_dir` and `external_map_path_for`. `source_dir` resolves `file` against `cwd`, then joins a relative `path` to the source file's directory. Public path semantics: `docs/content/apis/rust/Parser.mdx` and `docs/content/apis/javascript/createParser.mdx`
- code-entry functions (`parse`, `compile`) apply the resolved input map after a winning `parse` hook. The builtin parser resolves an input-map request carried by its own options
- `Generator::generate` applies `source_map.input` before code generation and never `source_map.output`. Output delivery remains a pipeline concern. `ResolvedGeneratorOptions.cwd` anchors a relative input-map path through the source file. Public contract: `docs/content/apis/rust/Generator.mdx`
- `generate_code` sets `map.file` after composing the input map. `apply_output_source_map_mode` overwrites it with a non-empty `output.file` after the winning generate result, including a custom map. Public contract: `docs/content/apis/index.mdx#source-maps`
- when `Inline` or `Auto` resolves an inline map, the builtin parser and `attach_input_map` remove the consumed comment from `Program.comments` before generation. `Ast::source` is unchanged. Comments that were not consumed remain
- the pipeline applies output options uniformly after the winning `generate` hook. This rule holds whether the builtin or a plugin generated the code. Public output semantics: `docs/content/apis/rust/compile.mdx#compileoptions` and `docs/content/apis/rust/generate.mdx#generateoptions`
- the builtin generator's output-map composition delegates to `oxc_sourcemap::SourceMap::compose`. The codegen map is the `self` operand, so it must contain at most one source
- when no map was produced, the code is returned byte-identical in every mode. Never fabricate an identity map

## Option resolution

Each entry point has its own options struct. The pipeline converts every struct to `ResolvableOptions` through the `From` impls in `_types/options/resolvable.rs`. It then passes the result through the plugin `options` fixpoint and resolves it to `ResolvedOptions` with defaults applied.

Parser and generator settings resolve into separate stages. `ResolvedOptions.parser` and `.generator` hold `ResolvedParserStageOptions` and `ResolvedGeneratorStageOptions`. The `From<&ResolvedOptions>` impls in `_types/options/resolved.rs` build the full `ResolvedParserOptions` / `ResolvedGeneratorOptions` for the builtin stages and their hooks. The builtin `Parser` and `Generator` store those resolved values. `parse_ast_with_parser_options` / `generate_code_with_codegen` and their program variants are the low-level entry points for caller-supplied raw oxc options. The default `parse_ast`, `parse_program`, `generate_code`, and `generate_program` delegate with the Telarel defaults. `telarel` exposes `Parser` and `Generator`; it does not re-export these low-level functions.

Raw `oxc` is a partial override patch, not a replacement base. Resolution runs in four layers: oxc defaults, Telarel defaults, the raw `oxc` patch, then curated fields. Only fields the caller sets override; an absent patch and an empty patch (`oxc: {}`) are no-ops. Curated fields and raw `oxc` fields are both user fields, and a curated field wins over its matching raw field. The effective `oxc` in `ResolvedParserStageOptions` / `ResolvedGeneratorStageOptions` is the full merged object, and the curated fields match it. One shared overlay helper per stage (`apply_curated_parser_options`, `apply_curated_generator_options`) applies curated fields for both the resolver and the builtin stages; `Parser::parse` and `Generator::generate` call them on their stored `oxc`, and the overlay is idempotent. `resolve_generator_stage_options` clears `oxc.source_map_path`; the builtin generator sets the path from `file`. Public semantics: [parser options](../../docs/content/apis/index.mdx#parser-options) and [generator options](../../docs/content/apis/index.mdx#generator-options).

Comments resolve as follows. Effective `minify` is the curated value, else the raw value, else `false`. Comments are enabled by default and disabled when effective `minify` is `true`. If the raw patch sets any comment category explicitly, those categories are user overrides and survive `minify`. Curated `comments: false` disables every category. Curated `comments: true` restores the default comment policy only when the effective raw comments equal `CommentOptions::disabled()`; otherwise it preserves the explicitly set raw categories. `ResolvedGeneratorStageOptions.comments` is `oxc.comments != CommentOptions::disabled()`.

The stage defaults are `preserve_parens = false`, `allow_return_outside_function = false`, `single_quote = false`, `ascii_only = false`, `indent_style = Space`, `indent_width = 2`, `comments = true` (or `false` when `minify` is `true`), and `minify = false`.

`default_parser_oxc`, `default_codegen_oxc`, and the `ResolvedParserStageOptions::from_oxc` / `ResolvedGeneratorStageOptions::from_oxc` constructors live in `_types/options/builtin/parser.rs` and `_types/options/builtin/generator.rs`. The `Default` impl of each stage type calls `from_oxc` with its own default function. `from_oxc` is the single place that derives the curated fields of a stage from `oxc`. The stage resolvers in `helpers/resolve.rs` start from `Resolved*StageOptions::default().oxc` and return the result of `from_oxc`.

`parse` and `rewrite` expose a flat `input_source_map` on their user options. Their conversions wrap it into the shared `source_map` bag with `output: None`. Plugin `options` hooks still receive the shared bag, and the parse and rewrite stages ignore an `output` that a hook sets. Public stage contract: `docs/content/apis/rust/parse.mdx` and `docs/content/apis/rust/rewrite.mdx`.

All internal path joining goes through `helpers/path.rs`. Resolution is lexical and never canonicalizes. Internal code keeps path options as filesystem paths and converts them to strings only at a boundary.

`FILE_DEFAULT` and `strip_file_suffix` are public in `telarel_common` for the transform plugin's output-file derivation. `telarel` does not re-export them. Do not add JavaScript package exports.

## Testing

Source-map tests use `tempfile` for external maps.

```sh
cargo test -p telarel_common
```
