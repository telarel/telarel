# Source Maps

A source map is a v3 map with `version`, `mappings`, `sources`, `names`, and optional `file`, `sourceRoot`, `sourcesContent`, and `ignoreList` fields.

- `Ast.map` is the **input** map: how the AST's source was produced upstream
- `CodeResult.map` is the **generated output** map, when one is produced
- `map.sources` keeps the input sources. `map.file` names the generated output

## Entry-Point Routing

| Entry point       | Input request     | Output request     | Notes                                       |
| ----------------- | ----------------- | ------------------ | ------------------------------------------- |
| `compile`         | `sourceMap.input` | `sourceMap.output` | Applies input after the winning parse hook. |
| `parse`           | `inputSourceMap`  | none               | No output map.                              |
| `rewrite`         | `inputSourceMap`  | none               | No output map.                              |
| `generate`        | `sourceMap.input` | `sourceMap.output` | Applies input at the boundary.              |
| `compileFromAst`  | `sourceMap.input` | `sourceMap.output` | Applies input at the boundary.              |
| `createParser`    | `sourceMap.input` | none               | Resolved `ParserOptions`.                   |
| `createGenerator` | `sourceMap.input` | `sourceMap.output` | Applies both when used standalone.          |

The `createGenerator` row is JavaScript. In Rust, `Generator::generate` applies `source_map.input` but not `source_map.output`; output delivery is a pipeline concern.

In Rust, `parse` and `rewrite` expose `input_source_map`, presented as `source_map.input` in the shared options bag. A plugin-set `source_map.output` or `output` is ignored.

Inside a `generate` hook, `args.options.sourceMap` / `source_map` has no input or output request. Pass `args.options` unchanged to `createGenerator` or `Generator::new`.

## Input Requests

JavaScript:

```ts
type InputSourceMap =
    | SourceMap
    | "inline"
    | "external"
    | "auto"
    | { mode: SourceMap | "inline" | "external" | "auto"; path?: string };
```

Rust:

```rust
enum InputSourceMapMode {
    Value(SourceMap),
    Inline,
    External,
    Auto,
}

enum InputSourceMap {
    Mode(InputSourceMapMode),
    Options(InputSourceMapOptions),
}
```

Modes:

- a `SourceMap` value is used directly
- `"inline"` reads a `//# sourceMappingURL=data:...` comment
- `"external"` reads an external map file
- `"auto"` tries the inline comment first, then the external map
- an omitted request means no lookup

Path rules:

- an explicit `path` is used for `external` and the `auto` fallback
- an absolute path is used as-is
- a relative path resolves against the source file's directory, not `cwd`
- without `path`, the default is `<file>.map`, with any `?query` or `#hash` suffix removed
- remote maps are never fetched. Pass a `SourceMap` value
- an `external` miss or corrupt map rejects
- an `inline` or `auto` miss keeps the AST's own input map
- an explicit request wins over the AST's own input map when it resolves

When an inline map is consumed, Telarel removes the consumed `sourceMappingURL` comment from `Program.comments`. `Ast.source` stays unchanged.

```ts
await compile(code, {
    file: "src/app.ts",
    sourceMap: { input: "auto" },
    plugins: [transform()],
});
```

## Output Requests

JavaScript:

```ts
type OutputSourceMap =
    | "inline"
    | "external"
    | "both"
    | { mode: "inline" | "external" | "both"; url?: string };
```

Rust:

```rust
enum OutputSourceMapMode {
    Inline,
    External,
    Both,
}

enum OutputSourceMap {
    Mode(OutputSourceMapMode),
    Options(OutputSourceMapOptions),
}
```

| Request         | Map returned | `sourceMappingURL` comment           |
| --------------- | ------------ | ------------------------------------ |
| omitted         | yes          | none                                 |
| `"inline"`      | no (`null`)  | data URI                             |
| `"external"`    | yes          | external URL                         |
| `"both"`        | yes          | data URI                             |
| `{ mode, url }` | per mode     | `url` for `external`, when non-empty |

Rules:

- no output request returns the map and leaves the code unchanged
- `"external"` appends `//# sourceMappingURL=<url>`
- the default external URL is `<output.file>.map`
- if `output.file` is empty or unset, the default URL uses the resolved input `file`; if both are empty, it is `index.ts.map`
- an explicit non-empty `sourceMap.output.url` wins over the default
- an empty `url` is treated as unset
- the compiler does not write the map file. It returns the map and appends the comment
- a non-empty `output.file` overwrites `map.file`, including a map returned by a custom `generate` hook
- if no map is produced, the code stays byte-identical in every mode. Telarel does not fabricate an identity map

```ts
const result: CodeResult = await compile(source, {
    file: "src/app.ts",
    output: { file: "dist/app.js" },
    sourceMap: { output: "external" },
    plugins: [transform()],
});

// result.code ends with:
// //# sourceMappingURL=dist/app.js.map
```

## Input Precedence

1. an explicit input request that resolves to a map
2. the AST's own `Ast.map`
3. no map

An explicit `inline` or `auto` request that finds no map keeps the AST's own map. An explicit `external` request that cannot read its map rejects.

For `compile`, the request applies to the AST returned by the winning `parse` hook. A map returned by a `transform` hook is the downstream result and is not replaced by the request.

## Common Mistakes

- replacing `ast.map` when you mean to request an input map
- passing `inputSourceMap` to `compile`, `generate`, or `compileFromAst`; use `sourceMap.input`
- forgetting that an `auto` input request may read an inline map and consume its comment
