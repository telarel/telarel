# JavaScript and TypeScript Plugins

A JavaScript plugin is a plain object with a required `name` and any subset of the eight hooks. Public contract: [JavaScript plugin API](https://telarel.github.io/apis/plugin/javascript).

## Imports

```ts
import type {
    AstResult,
    CodeResult,
    CommonPluginContext,
    FinalizeArgs,
    GenerateArgs,
    OptionsArgs,
    ParseArgs,
    Parser,
    Plugin,
    PluginContext,
    PrepareArgs,
    TransformArgs,
    TransformResult,
} from "telarel";

import { Ast, ErrorSource, PluginError, compile, createParser } from "telarel";
import { walk } from "telarel/walker";
```

## Plugin Shape

```ts
const plugin: Plugin = {
    name: "my-plugin",
    transform(_: PluginContext, args: TransformArgs): TransformResult {
        return { ast: args.ast };
    },
};
```

Hooks are return-based. Return the replacement value, or `null`/`void` for no change. `start`, `prepare`, `finalize`, and `end` are observers; their returns are ignored.

## `Ast`

- `ast.program` is deeply read-only; use `ast.mutate`
- `ast.source` is the source text the spans anchor to; `ast.language` is the grammar or `undefined`; `ast.map` is the **input** map
- only `transform` carries `mutate` changes forward; return `{ ast }`
- `prepare`, `finalize`, and `generate` receive snapshots; mutations do not reach the compiler
- to change source text, build a new `Ast` with the new `program` and `source`
- an AST returned from `parse` or `transform` owns its comments; returning one without comments drops them

## Walking the AST

```ts
ast.mutate((program): void => {
    walk(program, {
        enter(node): void {
            if (
                node.type === "ImportDeclaration" &&
                node.source.value === "react"
            ) {
                node.source.value = "preact/compat";
            }
        },
    });
});
```

`walk` is the `oxc-walker` walker re-exported by `telarel/walker`; it supports `enter` and `leave`.

## Recipes

### Rewrite imports

```ts
const importRewrite: Plugin = {
    name: "import-rewrite",
    transform(_ctx: PluginContext, { ast }: TransformArgs): TransformResult {
        ast.mutate((program): void => {
            walk(program, {
                enter(node): void {
                    if (
                        node.type === "ImportDeclaration" &&
                        node.source.value === "react"
                    ) {
                        node.source.value = "preact/compat";
                    }
                },
            });
        });
        return { ast };
    },
};
```

Covers side-effect, default, named, and namespace imports. Leaves dynamic `import("react")` and specifiers such as `"react/jsx-runtime"` alone.

### Adjust options

```ts
const optionsPlugin: Plugin = {
    name: "options-plugin",
    options: (_ctx: CommonPluginContext, args: OptionsArgs): OptionsArgs => ({
        options: {
            ...args.options,
            file: "custom.ts",
            plugins: [...(args.options.plugins ?? []), tracePlugin],
        },
    }),
};
```

The returned bag replaces the whole bag; omitted fields take defaults. Omitting `plugins` empties the list. Entries may be normalized descriptors; check `name`, not identity. Append a stable instance: creating the injected plugin inside the hook is fine, but an instance created on every pass hits the depth guard.

### Parse a custom input language (exclusive)

```ts
const greeting: Plugin = {
    name: "greeting",
    parse: async (
        _ctx: PluginContext,
        args: ParseArgs,
    ): Promise<AstResult | null> => {
        if (!args.code.startsWith("greet ")) {
            return null;
        }

        const name: string = args.code.slice("greet ".length).trim();

        const js: string = `console.log(${JSON.stringify(`hi ${name}`)});`;

        const parser: Parser = createParser(js, {
            ...args.options,
            language: "js",
        });

        return await parser.parse();
    },
};
```

- `parse` replaces the builtin parser. Only declare it when the plugin owns the input language; every later `parse` hook is skipped when it returns an AST
- return `null` for source that is not yours
- the returned AST's source is the translated JavaScript. To point spans at the original, build `new Ast({ program, source: args.code })`
- throw `PluginError` with `ErrorSource` for malformed custom source. Do not use `parse` for validation; use `prepare`

### Generate a custom output format (exclusive)

```ts
const manifest: Plugin = {
    name: "manifest",
    generate: (_ctx: PluginContext, { ast }: GenerateArgs): CodeResult => {
        const statements: Array<string> = ast.program.body.map(
            (node): string => node.type,
        );

        return {
            code: JSON.stringify({ statements }),
            map: null,
        };
    },
};
```

- `generate` replaces the builtin generator. Only declare it when the plugin owns the output format; every later `generate` hook is skipped when it returns code
- return `null`/`void` to defer. `args.options.sourceMap` is empty. `args.ast` is a snapshot
- the output string need not be JavaScript. A custom generator can return `map: null` when it does not produce a source map
- do not wrap or post-process builtin output here; post-process `CodeResult.code` at the call site

### Report an error with a source

```ts
const noDebugger: Plugin = {
    name: "no-debugger",
    prepare: (ctx: PluginContext, args: PrepareArgs): void => {
        const start: number = args.ast.source.indexOf("debugger");

        if (start === -1) {
            return;
        }

        throw new PluginError("debugger statements are not allowed", {
            source: ErrorSource.fromAst(ctx.module.file, args.ast, {
                start,
                end: start + "debugger".length,
            }),
        });
    },
};
```

`PluginError` surfaces as a `CompileError` with the same message and source; `render()` adds a code frame.

## Ordering

```ts
const ordered: Plugin = {
    name: "ordered",
    prepare: {
        order: "pre",
        handler: (_ctx: PluginContext, _args: PrepareArgs): void => {},
    },
    finalize: {
        order: "post",
        handler: (_ctx: PluginContext, _args: FinalizeArgs): void => {},
    },
};
```

Ordering is per hook. `pre` runs before every normal plugin, `post` after; the normal bucket keeps registration order.

## Passing Plugins

A `PluginOption` can be a plugin, a builtin descriptor, a falsy slot, a nested array, or a promise. A bare function is rejected.

## Testing

```ts
import { compile } from "telarel";
import { expect, it } from "vitest";

it("rewrites react imports", async (): Promise<void> => {
    const result = await compile('import { useState } from "react";', {
        cwd: "/repo",
        file: "index.ts",
        plugins: [importRewrite],
    });

    expect(result.code).toContain('"preact/compat"');
    expect(result.code).not.toContain('"react"');
});
```

Test the smallest entry point that runs the hook, then the full `compile` path. Add one failure-path test for `PluginError` or malformed source.
