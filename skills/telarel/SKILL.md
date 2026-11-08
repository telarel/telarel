---
name: telarel
description: Use when calling Telarel from JavaScript/TypeScript or Rust, choosing compile, parse, rewrite, generate, or compileFromAst options, configuring the builtin transform plugin, or debugging output, source maps, language inference, or CompileError.
---

# Using Telarel

Telarel compiles source text or an in-memory AST. Every entry point takes an options value and an optional plugin list. `compile` runs parse, prepare, transform, finalize, generate. Plugins are optional. Without the transform plugin, `compile` parses and regenerates the syntax as written.

This skill does not cover plugin authoring, compiler internals, or `docs/content`, the public contract source of truth.

## Pick the Entry Point

| Need                                    | JavaScript        | Rust               | Returns      |
| --------------------------------------- | ----------------- | ------------------ | ------------ |
| Source text to generated code           | `compile`         | `compile`          | `CodeResult` |
| Source text to an AST                   | `parse`           | `parse`            | `AstResult`  |
| AST to a rewritten AST                  | `rewrite`         | `rewrite`          | `AstResult`  |
| AST to generated code                   | `generate`        | `generate`         | `CodeResult` |
| Existing AST through transform and code | `compileFromAst`  | `compile_from_ast` | `CodeResult` |
| Builtin parser without plugins          | `createParser`    | `Parser::new`      | `Parser`     |
| Builtin generator without plugins       | `createGenerator` | `Generator::new`   | `Generator`  |

`generate` does not run `transform` hooks. Use `compileFromAst`, or call `rewrite` then `generate`.

## Minimal Use

### JavaScript

```ts
import type { CodeResult } from "telarel";

import { compile } from "telarel";
import { transform } from "telarel/plugins/transform";

const result: CodeResult = await compile("const x: number = 1;", {
    file: "index.ts",
    plugins: [transform({ targets: ["es2015"] })],
});
```

Rust: see [references/rust.md](references/rust.md).

## Options at a Glance

| JavaScript   | Rust          | Default         | Purpose                                           |
| ------------ | ------------- | --------------- | ------------------------------------------------- |
| `cwd`        | `cwd`         | `process.cwd()` | Root for relative paths.                          |
| `file`       | `file`        | `index.ts`      | Input path and language hint.                     |
| `language`   | `language`    | inferred        | `js`, `ts`, `dts`, `jsx`, or `tsx`.               |
| `sourceType` | `source_type` | inferred        | `script`, `commonjs`, `module`, or `unambiguous`. |
| `output`     | `output`      | unset           | `output.file` names the generated code.           |
| `sourceMap`  | `source_map`  | off             | Input and output source-map requests.             |
| `plugins`    | separate arg  | `[]`            | Plugins to run.                                   |

Rules:

- an explicit option wins over AST metadata, which wins over the file extension
- `file` defaults to `index.ts`; a JavaScript snippet without `file` parses as TypeScript
- empty `file`, `output.file`, and source-map `url` values are unset. `output.file` only labels output: it becomes `map.file` and the default external map URL. No file is written
- `sourceType: "unambiguous"` leaves the module kind to inference; a concrete value overrides it
- `parse` and `rewrite` accept `inputSourceMap`, not `sourceMap`, and produce no output map
- `generate` has no `sourceType` option; `program.sourceType` owns the module kind, though an `options` hook can replace it before generation

### Options by Entry Point

| Entry point      | `language` | `sourceType` | `output` | Source map       | `plugins` |
| ---------------- | ---------- | ------------ | -------- | ---------------- | --------- |
| `compile`        | yes        | yes          | yes      | `sourceMap`      | yes       |
| `parse`          | yes        | yes          | no       | `inputSourceMap` | yes       |
| `rewrite`        | yes        | yes          | no       | `inputSourceMap` | yes       |
| `generate`       | yes        | no           | yes      | `sourceMap`      | yes       |
| `compileFromAst` | yes        | yes          | yes      | `sourceMap`      | yes       |

In Rust, use snake_case names and pass plugins as the third argument.

## Transform Plugin

The transform plugin is opt-in. `transform()` with no options strips TypeScript but keeps modern syntax and JSX. Set options to lower more:

```ts
transform({
    targets: ["es2015"],
    typescript: { useDefineForClassFields: false },
    jsx: { runtime: "automatic" },
    define: { __DEV__: "false" },
});
```

- `targets: []` means ES-next; no syntax lowering, but TypeScript is still stripped
- `typescript.useDefineForClassFields: true` is independent of `targets`. `inject` runs before `define`; `define` runs last
- `jsx.runtime: "preserve"` is the default; set `"classic"` or `"automatic"` to lower JSX

See [references/plugins/transform.md](references/plugins/transform.md) for every option and the output-name table.

## Source Maps

`Ast.map` / `ast.map()` is the **input** map. `CodeResult.map` is the generated output map.

- no output request: return the map with no comment. `"inline"`: embed the map, return `map: null`. `"external"`: return the map and append the comment. `"both"`: embed the comment and return the map
- an `external` input request that cannot read its map rejects. An `inline` or `auto` miss keeps the AST's own map

See [references/source-maps.md](references/source-maps.md) for path resolution, precedence, and the full mode table.

## Errors

JavaScript entry points reject with `CompileError`:

```ts
import { CompileError, compile } from "telarel";

try {
    await compile("const x: = 1;");
} catch (error: unknown) {
    if (error instanceof CompileError) {
        console.error(error.render());
    }
}
```

`error.message` is the bare message. `error.source` holds the file, source text, and span. `error.render()` adds a code frame. Rust entry points return `Result<_, CompileError>` with the same message and optional source. Malformed JavaScript plugin values fail before the pipeline runs as `TypeError`, not `CompileError`.

## Common Mistakes

- expecting `compile` alone to lower TypeScript or modern syntax. Pass `transform()`
- passing `sourceMap` to `parse` or `rewrite`. Use `inputSourceMap`
- expecting `sourceMap.output: "inline"` to return a map. It returns `null`
- expecting `output.file` to write a file. It only labels the output
- using `generate` and expecting transform hooks to run. Use `compileFromAst`, or `rewrite` then `generate`

## References

| Topic                   | Reference                                                          |
| ----------------------- | ------------------------------------------------------------------ |
| JavaScript entry points | [references/javascript.md](references/javascript.md)               |
| Rust entry points       | [references/rust.md](references/rust.md)                           |
| Transform plugin        | [references/plugins/transform.md](references/plugins/transform.md) |
| Source-map contract     | [references/source-maps.md](references/source-maps.md)             |
| Public API              | <https://telarel.github.io/apis>                                   |

Read the language reference before writing code.
