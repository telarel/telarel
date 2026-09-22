# Architecture

This is a architecture documentation of the compiler.

## Overview

Telarel is an extensible JavaScript compiler written in Rust, powered by [oxc](https://oxc.rs) and built around plugin hooks.

The [`compile`](./crates/core/src/lib.rs#L61) function takes compile options (`cwd`, `file`, `code`) plus a list of plugins, and returns a `CompileOutput` which generated code with a source map.

## Dependencies

```mermaid
flowchart TD
    common --> plugin
    plugin --> core
    common --> core
    common --> binding
    plugin --> binding
    core --> binding
    common --> telarel
    plugin --> telarel
    core --> telarel
```

`common` has no dependencies.

`binding` and `telarel` both depend on `common`, `plugin`, and `core`.

There are no cycles.

## Pipeline

```mermaid
flowchart TD
    Options[<b>options</b><br/>field-by-field merge of options]
    Pre[<b>pre</b><br/>notify-all; first error aborts]
    TransformNeeded{any transform plugin?}
    Parse[parse into allocator]
    Transform[<b>transform</b><br/>chained in-place mutation]
    Codegen[codegen: code + map]
    Skip[skip parse<br/>codegen: map]
    Post[<b>post</b><br/>notify-all; first error aborts]
    Output([code + map])

    Options --> Pre --> TransformNeeded
    TransformNeeded -->|yes| Parse --> Transform --> Codegen --> Post
    TransformNeeded -->|no| Skip --> Post
    Post --> Output
```

The plugin implements 4 hooks (all with no operation by default). `options` goes without context since the context is built from the resolved options.

| Hook      | Context | Arguments                          | Return          | Merge                      |
| --------- | ------- | ---------------------------------- | --------------- | -------------------------- |
| options   | –       | options                            | partial options | last `Some` wins per field |
| pre       | ✓       | file, code (read-only)             | `()`            | notify                     |
| transform | ✓       | allocator, file, program (mutable) | `()`            | chained; in-place mutation |
| post      | ✓       | file, final code                   | `()`            | notify                     |

Merge semantics:

- **Last `Some` wins per field** — the options hook merges each returned [`PartialCompileOptions`](./crates/common/src/_types/options/compile.rs#L14) field-by-field into the carried options; `None` fields keep their current values, and later plugins observe earlier merges
- **Notify** — `pre` and `post` run for their side effects in registration order; the first error aborts the run
- **Chained** — each `transform` plugin mutates the same [`Program`](./crates/plugin/src/_types/hooks/transform.rs#L6) in place, so mutations are visible to the following plugins; a plugin may swap the entire root by assigning `*args.program` with a tree allocated in the shared allocator — pointer stability is the [driver's](./crates/plugin/src/plugin_driver/hooks/transform.rs) job

### Hook Usage Declaration

[`register_hook_usage`](./crates/plugin/src/plugin/mod.rs#L22) is required and affects how the driver runs plugins. If a hook isn't declared, it won't be called.

On the JS side, usage is inferred from the hook properties on the plugin object. Therefore, even if `transform` never mutates the program, it still triggers parse + codegen.

### Parse Skip

The driver's aggregate [`HookUsage`](./crates/common/src/_types/hooks/usage.rs#L6) determines whether the source needs to be parsed. If no plugin declares `Transform`, parsing and codegen are skipped entirely. In that case, even invalid syntax is passed through, a [per-line identity map](./crates/core/src/lib.rs#L31) is returned instead of producing a parse error.

Declaring only `pre` or `post` does not trigger parsing.

## JS Plugin Bridge

The [binding](./crates/binding/src/plugin/pluginable.rs#L37) wraps JS plugin objects in `Pluginable` implementations that call JS through thread-safe function calls (TSFNs). The AST crosses the boundary as a JSON-serialized ESTree string via `oxc_estree_codec`:

```mermaid
flowchart LR
    P["Rust: Program"] -- serialize --> S["JSON string"] -- parse --> E["JS: ESTree AST"]
    E -- stringify --> S -- deserialize --> P
```

The JS `transform` hook is mutation-based: the wrapper parses the serialized tree, calls the hook, re-stringifies the tree, and compares it with the pre-call string. Equal — nothing crosses back into Rust; different — the JSON is parsed back into the compile allocator and swapped in as the root. Returning any value from a JS `transform` hook throws a `TypeError`. A shared `metadata` object is created for each compile and released in `Task::finally`, so it remains isolated between different compiles.

## Execution Model

The transform hook's future is intentionally non-`Send` (`LocalHookFuture`) because programs borrow from the compile allocator. On native, each `compile` call runs on a libuv worker thread and `block_on`s a per-thread, reused current-thread Tokio runtime ([`block_on_compile`](./crates/binding/src/tasks/mod.rs), used by [`CompileTask`](./crates/binding/src/tasks/compile.rs#L10)). The non-`Send` transform future never escapes the worker thread: it is fully driven and dropped inside `block_on`.

The same TSFN-based binding also works with `wasm32-wasip1-threads`, and the JS test suite runs against both backends.

For traversal, plugin authors choose by task:

| Task                        | Rust                                          | JS                                           |
| --------------------------- | --------------------------------------------- | -------------------------------------------- |
| Simple field edits          | `telarel::ast_visit::{VisitMut, walk_mut}`    | mutate `args.ast`, or `telarel/walker`       |
| Parent/scope-aware rewrites | `telarel::traverse::{Traverse, traverse_mut}` | `walk` with `this.replace()` / `this.skip()` |

Details:

- **Simple field edits** — rename, retag, drop a node. JS `walk` is backed by `oxc-walker`.
- **Parent/scope-aware rewrites** — insert after a node, rename the binding a reference resolves to. Rust: enable the `traverse` feature; [`TraverseCtx`](./crates/telarel/src/lib.rs) provides parent/ancestor access and an `AstBuilder` for allocating nodes; scoping is built per compile via `SemanticBuilder::new().build(program).semantic.into_scoping()`. JS: add a scope tracker only if the transform does not replace nodes.
