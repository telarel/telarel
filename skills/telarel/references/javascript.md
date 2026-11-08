# JavaScript and TypeScript Usage

Public contract: [JavaScript API](https://telarel.github.io/apis/javascript/compile).

## Install and Imports

```sh
npm install telarel   # or: pnpm add telarel
```

```ts
import type {
    AstResult,
    CodeResult,
    Generator,
    Parser,
    PluginOption,
} from "telarel";

import {
    Ast,
    CompileError,
    PluginError,
    compile,
    compileFromAst,
    createGenerator,
    createParser,
    generate,
    parse,
    rewrite,
} from "telarel";
import { transform } from "telarel/plugins/transform";
import { walk } from "telarel/walker";
```

`telarel` also re-exports `transform`; the documented entry point is `telarel/plugins/transform`. Every entry-point option type also accepts `cwd` (default `process.cwd()`); `CompileOptions` lists it.

## `compile`

```ts
function compile(code: string, options?: CompileOptions): Promise<CodeResult>;
```

Runs parse, prepare, transform, finalize, and generate.

```ts
const result: CodeResult = await compile("const x: number = 1;", {
    file: "src/index.ts",
    plugins: [transform({ targets: ["es2015"] })],
});
```

### `CompileOptions`

| Field        | Type               | Default         | Description                                       |
| ------------ | ------------------ | --------------- | ------------------------------------------------- |
| `cwd`        | `string`           | `process.cwd()` | Current working directory.                        |
| `file`       | `string`           | `index.ts`      | Input file. The extension infers the grammar.     |
| `language`   | `Language`         | inferred        | Grammar. Overrides extension inference.           |
| `sourceType` | `SourceType`       | inferred        | Module kind. Overrides the grammar's module kind. |
| `output`     | `OutputOptions`    | unset           | `{ file?: string }`. Labels the generated code.   |
| `sourceMap`  | `SourceMapOptions` | off             | Input and output source-map requests.             |
| `plugins`    | `PluginOption[]`   | `[]`            | Plugins to run.                                   |

Without the transform plugin, `compile` parses and regenerates the source but does not lower TypeScript, modern syntax, or JSX.

## `parse`

```ts
function parse(code: string, options?: ParseOptions): Promise<AstResult>;
```

Parses source text into an AST. Plugins can replace the parse result.

### `ParseOptions`

| Field            | Type             | Default    | Description               |
| ---------------- | ---------------- | ---------- | ------------------------- |
| `file`           | `string`         | `index.ts` | Input file.               |
| `language`       | `Language`       | inferred   | Grammar.                  |
| `sourceType`     | `SourceType`     | inferred   | Module kind.              |
| `inputSourceMap` | `InputSourceMap` | off        | Input source-map request. |
| `plugins`        | `PluginOption[]` | `[]`       | Plugins to run.           |

`parse` returns `{ ast }` and has no output map. `inputSourceMap` is normalized to `sourceMap.input` for the options hook. A plugin-set `sourceMap.output` or `output` is ignored.

## `rewrite`

```ts
function rewrite(ast: Ast, options?: RewriteOptions): Promise<AstResult>;

function rewrite(
    program: Program,
    code: string,
    options?: RewriteOptions,
): Promise<AstResult>;
```

Runs the rewrite stages on an AST: prepare, transform, finalize.

### `RewriteOptions`

| Field            | Type             | Default                    | Description               |
| ---------------- | ---------------- | -------------------------- | ------------------------- |
| `file`           | `string`         | `index.ts`                 | Input file.               |
| `language`       | `Language`       | AST language, then file    | Grammar.                  |
| `sourceType`     | `SourceType`     | AST module kind, then file | Module kind.              |
| `inputSourceMap` | `InputSourceMap` | off                        | Input source-map request. |
| `plugins`        | `PluginOption[]` | `[]`                       | Plugins to run.           |

A concrete `sourceType` replaces the AST module kind. `"unambiguous"` requests inference. `rewrite` does not reparse the AST and has no output map.

## `generate`

```ts
function generate(ast: Ast, options?: GenerateOptions): Promise<CodeResult>;

function generate(
    program: Program,
    code: string,
    options?: GenerateOptions,
): Promise<CodeResult>;
```

Generates code from an AST. It runs only the generate stage.

### `GenerateOptions`

| Field       | Type               | Default    | Description                                                              |
| ----------- | ------------------ | ---------- | ------------------------------------------------------------------------ |
| `file`      | `string`           | `index.ts` | Output file hint and final grammar fallback.                             |
| `language`  | `Language`         | inferred   | Grammar. Overrides the AST language; the file extension is the fallback. |
| `output`    | `OutputOptions`    | unset      | `{ file?: string }`. Labels the generated code.                          |
| `sourceMap` | `SourceMapOptions` | off        | Input and output source-map requests.                                    |
| `plugins`   | `PluginOption[]`   | `[]`       | Plugins to run.                                                          |

`generate` has no `sourceType` option. `ast.program.sourceType` owns the module kind; an `options` hook can still replace it before generation. An explicit `language` overrides the AST language for generation:

```ts
const tsx = await parse("const f = <T,>(x: T) => x;", { language: "tsx" });

const asTs: CodeResult = await generate(tsx.ast, { language: "ts" });
// const f = <T>(x: T) => x;
```

`generate` does not run `transform` hooks. Use `compileFromAst` for that.

## `compileFromAst`

```ts
function compileFromAst(
    ast: Ast,
    options?: CompileFromAstOptions,
): Promise<CodeResult>;

function compileFromAst(
    program: Program,
    code: string,
    options?: CompileFromAstOptions,
): Promise<CodeResult>;
```

Runs prepare, transform, finalize, and generate on an existing AST. It does not reparse.

### `CompileFromAstOptions`

| Field        | Type               | Default                    | Description                           |
| ------------ | ------------------ | -------------------------- | ------------------------------------- |
| `file`       | `string`           | `index.ts`                 | Input file.                           |
| `language`   | `Language`         | AST language, then file    | Grammar.                              |
| `sourceType` | `SourceType`       | AST module kind, then file | Module kind.                          |
| `output`     | `OutputOptions`    | unset                      | `{ file?: string }`.                  |
| `sourceMap`  | `SourceMapOptions` | off                        | Input and output source-map requests. |
| `plugins`    | `PluginOption[]`   | `[]`                       | Plugins to run.                       |

The input AST's source text and spans must stay consistent. Build a new `Ast` when you need different source text.

## Builtin Constructors

`createParser` and `createGenerator` call the builtin stages directly. They do not run the plugin chain and take fully resolved options, so every field is required. `ParserOptions` has `cwd`, `file`, `language`, `sourceType`, and `sourceMap`; `GeneratorOptions` has `cwd`, `file`, and `sourceMap`.

`createGenerator` applies `sourceMap.input` and `sourceMap.output` standalone. Inside a `generate` hook, pass `args.options` unchanged; the pipeline has resolved the input and owns output delivery.

## `Ast`

`Ast` carries the ESTree program, the source text its spans anchor to, optional language metadata, and an optional input source map. Construct it with `new Ast({ program, source, language?, map? })`.

- `ast.program` is deeply read-only. Use `ast.mutate` to change it
- `ast.source` is the source text the spans anchor to
- `ast.language` is the grammar, or `undefined` when unknown
- `ast.sourceType` reads `ast.program.sourceType`
- `ast.map` is the **input** map, never the generated map

```ts
ast.mutate((program): void => {
    program.body.length = 0;
});
```

Only a `transform` hook carries `mutate` changes forward; return `{ ast }` to keep them.

## Passing Plugins

A `PluginOption` can be a plugin object, a builtin descriptor, a falsy slot, a nested array, or a promise. A bare function is rejected.

## Errors

`CompileError` carries `message`, optional `source`, and `render()`. A `PluginError` thrown by a hook reaches the entry point as a `CompileError` with the same message and source.
