# Telarel Hook Contract

Shared contract for JavaScript/TypeScript and Rust plugins. Full reference: [Plugin API](https://telarel.github.io/apis/plugin).

## Hook Summary

| Hook        | Arguments                             | Return                         | Semantics    |
| ----------- | ------------------------------------- | ------------------------------ | ------------ |
| `options`   | context, resolvable options + plugins | replacement options or nothing | Fixpoint     |
| `start`     | context, resolved options             | ignored                        | Notify       |
| `parse`     | context, code + parser options        | AST result or nothing          | First-win    |
| `prepare`   | context, AST                          | ignored                        | Observer     |
| `transform` | context, owned AST handle             | replacement AST or nothing     | Carried fold |
| `finalize`  | context, AST                          | ignored                        | Observer     |
| `generate`  | context, AST + generator options      | code result or nothing         | First-win    |
| `end`       | context, error or nothing             | ignored                        | Notify       |

Rust return types: `ParseReturn`, `TransformReturn`, `GenerateReturn`, `OptionsReturn`, and `NotifyReturn`. Every fallible Rust hook returns `Result<_, PluginError>`.

Deferrable hooks (`options`, `parse`, `transform`, `generate`) use `Ok(None)` / `null` / `void` to make no change. Notify hooks (`start`, `prepare`, `finalize`, `end`) use `Ok(())`; their success value is ignored, but an error still fails the stage.

## `options`: Fixpoint Replace

- runs before defaults resolve, as a sequential fixpoint
- identity is the plugin instance, not the name: Rust `Arc::ptr_eq`; JavaScript object identity. Each distinct instance runs once; duplicate entries share one run
- a returned bag replaces the whole bag at struct level; omitted fields take defaults, not previous values. Returning nothing keeps the current bag
- payload: resolvable options plus the plugin list; never source text
- a returned plugin list replaces the settled list wholesale, including same-length swaps, reorders, and truncations
- injected plugins get their own `options` hook on a later pass. An instance created every pass is new work every pass; the fixpoint rejects at `MAX_OPTIONS_DEPTH` (10)

In JavaScript, `args.options.plugins` may contain bridge-normalized descriptors, not the original objects. Check `name`, not identity. In Rust, `args.plugins` holds shared handles.

## `parse`: First-Win (Exclusive Replacement)

- the first AST result wins; every later `parse` hook is skipped, and the builtin parser does not run. Returning nothing defers; if all defer, the builtin parser runs
- declare it only when the plugin owns the input language. Return nothing for source that is not yours. Do not use `parse` for validation; use `prepare`
- after a win, the entry point's input source-map request applies to the returned AST: an explicit request wins when it resolves; an `inline`/`auto` miss keeps the AST map; an `external` miss rejects
- the returned AST owns its source text and its spans anchor to it; it may differ from the entry-point code

## `prepare` and `finalize`: Observers

- `prepare` observes the parsed AST; `finalize` observes the final AST after transform
- return values are ignored; neither can change the tree
- in JavaScript, the hook receives an `Ast` snapshot (mutations stay local); in Rust, it receives `&Ast`

## `transform`: Carried Fold

- one owned AST passes plugin to plugin; the last replacement wins
- returning nothing leaves the carried AST untouched; an unreturned mutation is discarded
- in Rust: `Ast::with_mut` copies the arena on write because the driver holds its own handle; return `Some(TransformOutput { ast })`
- in JavaScript: `Ast.mutate`, then return `{ ast }`; `null`/`void` discards
- `with_mut` cannot change the source text. `Program::source_text` must stay content-equal to `Ast::source` (debug builds assert). To change it, build a new `Ast`; in JavaScript, construct one with the new `program` and `source`

## `generate`: First-Win (Exclusive Replacement)

- the first code result wins; every later `generate` hook is skipped, and the builtin generator does not run. Returning nothing defers; if all defer, the builtin generator runs
- declare it only when the plugin owns the output format. Do not use `generate` to wrap or post-process builtin output; post-process `CodeResult.code` at the call site
- generator options carry an empty source-map request: input resolution precedes the hook, output delivery follows the winning result
- in Rust, the result is `CodeResult { code, map }`; in JavaScript, `{ code, map? }`

## `start` and `end`: Notify

- `start` runs on every settled entry with fully resolved, read-only options; return values are ignored
- `end` runs on every settled entry on every path, including failure, but not when the `options` stage fails
- if a stage fails and `end` fails, the `end` error replaces the stage error; on success, the `end` error is returned. Duplicate entries are included
- `end` error: JavaScript `string | null`; Rust `Option<CompileError>`

## Entry-Point Matrix

| Hook        | `parse` | `rewrite` | `generate` | `compileFromAst` | `compile` |
| ----------- | ------- | --------- | ---------- | ---------------- | --------- |
| `options`   | yes     | yes       | yes        | yes              | yes       |
| `start`     | yes     | yes       | yes        | yes              | yes       |
| `parse`     | yes     | no        | no         | no               | yes       |
| `prepare`   | no      | yes       | no         | yes              | yes       |
| `transform` | no      | yes       | no         | yes              | yes       |
| `finalize`  | no      | yes       | no         | yes              | yes       |
| `generate`  | no      | no        | yes        | yes              | yes       |
| `end`       | yes     | yes       | yes        | yes              | yes       |

In Rust, `compileFromAst` is `compile_from_ast`. A hook outside the entry point's stage plan never runs; `generate(ast)` does not run `transform`. Use `compileFromAst` or `rewrite` then `generate`.

## Ordering

Each hook has its own optional order; a plugin can be `pre` for one hook and `post` for another.

| Order  | Runs                              |
| ------ | --------------------------------- |
| `pre`  | before every normal plugin        |
| unset  | normal bucket, registration order |
| `post` | after every normal plugin         |

- in JavaScript: `ObjectHook<T>` is the handler or `{ order?: "pre" | "post"; handler: T }`
- in Rust: each hook has a `<hook>_meta` method returning `Option<PluginHookMeta>`
- stable within a bucket. `options` re-ranks a copy each pass; the persisted list keeps registration order. `start`/`end` use their meta but run on every settled plugin

## Context

- `options` receives `CommonPluginContext` (no fields)
- later hooks receive `PluginContext`: `cwd` (filesystem root) and `module` (`file`, `language`, `source_type` / `sourceType`)
- in JavaScript, non-UTF-8 path values render lossily with `U+FFFD`
- the pipeline resolves `cwd`, `file`, `language`, and `sourceType` before any `PluginContext` hook. `options` sees unset values as absent, not empty strings

## Errors

- in JavaScript: throw `PluginError` with a message and optional `ErrorSource`. Plain errors fail the compile but lose the structured location
- in Rust: `Err(PluginError::new(message))`; attach a source with `with_source_ast`, `with_source_code`, or `with_source`
- a hook error reaches the entry point as a `CompileError` with message/source unchanged. An `end` error replaces a stage error

## Source Maps

- `Ast::map()` / `Ast.map` is the input map, never the generated output map
- an explicit input request wins over the AST map when it resolves; an `inline`/`auto` miss keeps the AST map; an `external` miss rejects. It applies to the AST from the winning `parse` hook
- a `generate` hook's options carry an empty request (`{ input: null, output: null }` in JavaScript). The pipeline owns input resolution and output delivery
- after the winning `generate` result, a non-empty `output.file` overwrites `map.file`, including a hook map. An external map appends `sourceMappingURL` using a non-empty `sourceMap.output.url` or `<output.file>.map`. No map means byte-identical code and no comment

## What Each Hook Can See

| Hook        | Source text | Parsed AST | Final AST | Options visible  | Output request |
| ----------- | ----------- | ---------- | --------- | ---------------- | -------------- |
| `options`   | no          | no         | no        | resolvable bag   | yes            |
| `start`     | no          | no         | no        | resolved options | yes            |
| `parse`     | yes         | no         | no        | parser options   | input only     |
| `prepare`   | via AST     | yes        | no        | context only     | no             |
| `transform` | via AST     | yes        | no        | context only     | no             |
| `finalize`  | via AST     | no         | yes       | context only     | no             |
| `generate`  | via AST     | no         | yes       | generator opts   | empty request  |
| `end`       | no          | no         | no        | context only     | no             |

`options` cannot see the source text or AST language. When the builtin transform plugin derives `output.file` in `compileFromAst`, set `output.file` explicitly if the AST language differs from the grammar implied by `file` and `language`.
