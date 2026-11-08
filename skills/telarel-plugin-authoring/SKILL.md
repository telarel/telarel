---
name: telarel-plugin-authoring
description: Use when writing or reviewing a Telarel plugin in JavaScript/TypeScript or Rust, choosing among the options, start, parse, prepare, transform, finalize, generate, and end hooks, or debugging a hook that is skipped, does not change the AST, or returns the wrong value.
---

# Writing Telarel Plugins

A Telarel plugin is a named value with any subset of eight hooks. The compiler owns pipeline state. A hook returns a replacement for the current value or observes it. Pick the narrowest hook, return the documented shape, and test with `compile` or `rewrite`.

This covers consumers of the public `telarel` npm package and Rust crate, not compiler internals or builtin plugins.

## When to Use

- writing a JavaScript/TypeScript plugin object or Rust `Plugin` impl
- choosing between `transform`, `parse`, `generate`, `options`, and observers
- composing or ordering plugins, or debugging a skipped, deferred, or no-op hook

## When Not to Use

- changing Telarel core, the builtin transform plugin, or the plugin driver
- only calling entry points or configuring options

## Which Hook?

| Goal                              | Hook        | Return           | Key rule                                               |
| --------------------------------- | ----------- | ---------------- | ------------------------------------------------------ |
| Change options before resolution  | `options`   | options bag      | Full-bag replacement; identity is the instance         |
| Setup, metrics, logging           | `start`     | nothing          | Receives resolved options                              |
| Parse a custom input language     | `parse`     | `{ ast }`        | Exclusive; skips later hooks and the builtin parser    |
| Inspect the parsed AST            | `prepare`   | nothing          | Observer; mutations do not persist                     |
| Rewrite the AST                   | `transform` | `{ ast }`        | Carried fold; return the AST to carry changes          |
| Inspect the final AST             | `finalize`  | nothing          | Observer; mutations do not persist                     |
| Generate a custom output format   | `generate`  | `{ code, map? }` | Exclusive; skips later hooks and the builtin generator |
| Cleanup, metrics, error reporting | `end`       | nothing          | Runs on success and failure                            |

`parse` and `generate` replace the builtin stage; they do not compose. Declare `parse` only when the plugin owns the input language, and `generate` only when it owns the output format. Return `null` / `None` to defer. Use `prepare` or `finalize` for validation, and `transform` for AST edits.

The hook contract is shared. See [references/hook-contract.md](references/hook-contract.md) for the entry-point matrix, ordering, `options` fixpoint, source maps, and error rules.

## Minimal JavaScript Plugin

```ts
import type {
    Plugin,
    PluginContext,
    TransformArgs,
    TransformResult,
} from "telarel";

import { compile } from "telarel";

const plugin: Plugin = {
    name: "clear-body",
    transform(_ctx: PluginContext, { ast }: TransformArgs): TransformResult {
        ast.mutate((program): void => {
            program.body.length = 0;
        });

        return { ast };
    },
};

await compile("const x: number = 1;", { plugins: [plugin] });
```

`Ast.mutate` changes the program. Returning `{ ast }` carries the change; returning `null` or `void` discards it. See [references/javascript.md](references/javascript.md).

## Minimal Rust Plugin

```rust
use std::borrow::Cow;
use std::future::Future;

use telarel::allocator::Allocator;
use telarel::ast::ast::Program;
use telarel::{
    Ast, HookUsage, Plugin, PluginContext, TransformArgs, TransformOutput,
    TransformReturn,
};

#[derive(Debug)]
struct ClearBody;

impl Plugin for ClearBody {
    fn name(&self) -> Cow<'static, str> {
        "clear-body".into()
    }

    fn register_hook_usage(&self) -> HookUsage {
        HookUsage::Transform
    }

    fn transform<'a>(
        &'a self,
        _ctx: &'a PluginContext<'_>,
        args: TransformArgs,
    ) -> impl Future<Output = TransformReturn> + Send + 'a {
        async move {
            let mut ast: Ast = args.ast;
            ast.with_mut(|_: &Allocator, program: &mut Program<'_>| {
                program.body.clear();
            });
            Ok(Some(TransformOutput { ast }))
        }
    }
}
```

Use `Plugin::new_shared(ClearBody)` in the plugin list. `HookUsage::Transform` is required; otherwise the driver skips the hook. See [references/rust.md](references/rust.md) for a complete walker recipe.

## Failure Modes

- `ast.program` is deeply read-only in JavaScript; use `ast.mutate`. In Rust, `with_mut` cannot change the source text
- a mutation that is not returned from `transform` is discarded: return `{ ast }` / `Ok(Some(TransformOutput { ast }))`
- mutating in `prepare`, `finalize`, or `generate` has no effect; those hooks receive snapshots
- an options bag replaces the whole bag. Forgetting `plugins` makes the plugin list empty
- `options` identity is the plugin instance, not the name. `args.options.plugins` may hold normalized descriptors; check `name`
- ordering is per hook, not per plugin. `parse` and `generate` are first-win and exclusive: a result skips later hooks of that name and the builtin stage. `transform` is a carried fold
- don't use `parse` or `generate` for validation or post-processing; use `prepare` / `finalize`, or post-process the returned `CodeResult` at the call site
- a `generate` hook's options carry an empty source-map request. A failing `end` hook replaces the stage error
- in Rust, declaring `HookUsage::Transform` is required; a fresh `OptionsArgs` must keep `args.plugins`

## References

| Topic                         | Reference                                                  |
| ----------------------------- | ---------------------------------------------------------- |
| Shared hook contract          | [references/hook-contract.md](references/hook-contract.md) |
| JavaScript/TypeScript surface | [references/javascript.md](references/javascript.md)       |
| Rust surface                  | [references/rust.md](references/rust.md)                   |
| Public Plugin API             | <https://telarel.github.io/apis/plugin>                    |
| Source maps                   | <https://telarel.github.io/apis#source-maps>               |

Read the language reference before writing code.
