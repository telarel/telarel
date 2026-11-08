# Transform Plugin

The transform plugin wraps the oxc transformer. It strips TypeScript, lowers modern syntax to `targets`, lowers JSX only when configured, and applies `inject` and `define`.

- in JavaScript: `transform(options?)` from `telarel/plugins/transform`
- in Rust: `TransformPlugin::with_options(options)` from `telarel_plugin_transform`; `TransformPlugin::new()` uses defaults

## `TransformOptions`

| Option          | JavaScript                    | Rust                          | Default |
| --------------- | ----------------------------- | ----------------------------- | ------- |
| `targets`       | `TransformTarget[]`           | `Vec<TransformTarget>`        | `[]`    |
| `typescript`    | `TypeScriptOptions`           | `Option<TypeScriptOptions>`   | unset   |
| `jsx`           | `JsxOptions`                  | `Option<JsxOptions>`          | unset   |
| `inject`        | `Record<string, ...>`         | `Option<InjectOptions>`       | unset   |
| `define`        | `Record<string, DefineValue>` | `Option<DefineOptions>`       | unset   |
| `oxc`           | `OxcTransformOptions`         | `Option<OxcTransformOptions>` | unset   |
| `helper_loader` | not exposed                   | `Option<HelperLoaderOptions>` | unset   |

## Targets

```ts
transform({ targets: ["es2015", { chrome: 120 }, { node: 22 }] });
```

- editions: `es2015` through `es2026`, plus `esnext`
- fixed runtimes: `hermes` and `rhino`
- engine versions: `chrome`, `deno`, `edge`, `firefox`, `ios`, `node`, `opera`, `safari`, `samsung`, and `electron`
- in Rust: `TransformTarget::Es2015`, `TransformTarget::Chrome(120)`, `TransformTarget::Hermes`, and so on
- `targets: []` means ES-next. The plugin still strips TypeScript
- `targets` wins over the raw `oxc.env` setting
- engine versions use the major component. Rust saturates versions above `u16::MAX`

## TypeScript

Field names follow `tsconfig` where the transform can support them.

| JavaScript                        | Rust                                 | Default | Behavior                                                                               |
| --------------------------------- | ------------------------------------ | ------- | -------------------------------------------------------------------------------------- |
| `rewriteRelativeImportExtensions` | `rewrite_relative_import_extensions` | `false` | Rewrites `.ts`, `.tsx`, `.mts`, `.cts` import extensions.                              |
| `verbatimModuleSyntax`            | `verbatim_module_syntax`             | `false` | Keeps value imports; removes only `type` specifiers.                                   |
| `emitDecoratorMetadata`           | `emit_decorator_metadata`            | `false` | Emits decorator metadata. Requires `experimentalDecorators`.                           |
| `experimentalDecorators`          | `experimental_decorators`            | `false` | Lowers legacy TypeScript decorators.                                                   |
| `useDefineForClassFields`         | `use_define_for_class_fields`        | `true`  | Define semantics. `false` removes uninitialized fields and assigns initialized fields. |
| `optimizeConstEnums`              | `optimize_const_enums`               | `false` | Inlines const enum members and removes unused declarations.                            |

- `useDefineForClassFields` does not depend on `targets` (unlike `tsc`). An explicit value overrides raw `oxc` class-field assumptions; otherwise an explicit raw setting wins over the default
- `emitDecoratorMetadata: true` requires `experimentalDecorators: true`
- with `verbatimModuleSyntax`, mark type-only re-exports with `export type`; oxc still elides a re-export specifier with no value reference
- `rewriteRelativeImportExtensions` rewrites every specifier containing `/`, not only relative ones, and maps `.tsx` to `.js` (`tsc` maps it to `.jsx`)
- `optimizeConstEnums` is a Babel-compatible extension, not a `tsconfig` option
- in classic JSX mode, `jsx.pragma` and `jsx.pragmaFrag` also drive JSX import elision

## JSX

| JavaScript     | Rust            | Default                 | Behavior                                     |
| -------------- | --------------- | ----------------------- | -------------------------------------------- |
| `runtime`      | `runtime`       | `"preserve"`            | `"preserve"`, `"classic"`, or `"automatic"`. |
| `importSource` | `import_source` | `"react"`               | Module for automatic-runtime helpers.        |
| `pragma`       | `pragma`        | `"React.createElement"` | Function for classic-runtime elements.       |
| `pragmaFrag`   | `pragma_frag`   | `"React.Fragment"`      | Component for classic-runtime fragments.     |
| `development`  | `development`   | `false`                 | Adds development-only JSX metadata.          |
| `refresh`      | `refresh`       | `false`                 | Enables React Fast Refresh instrumentation.  |

- `preserve` keeps JSX unchanged. It ignores `development`, `refresh`, `importSource`, `pragma`, and `pragmaFrag`
- raw `oxc.jsx` fields do not enable JSX lowering. Set `jsx.runtime` to `"classic"` or `"automatic"` explicitly
- in Rust: `JsxRuntime::Preserve`, `JsxRuntime::Classic`, and `JsxRuntime::Automatic`

```ts
transform({ jsx: { runtime: "automatic" } });
transform({ jsx: { runtime: "classic", pragma: "h", pragmaFrag: "Fragment" } });
```

## `inject`

`inject` adds imports for identifiers that would otherwise be free. The key is the local binding name. Dotted keys such as `"Object.assign"` are replaced as member chains.

Forms:

- string value: emits `import { default as <local> } from "<source>"`
- `[source, imported]` pair: emits `import { <imported> as <local> } from "<source>"`
- object form:
    - `mode: "named"` (default) with `imported` emits `import { <imported> as <local> } from "<source>"`
    - `mode: "named"` without `imported` emits `import { default as <local> } from "<source>"`
    - `mode: "default"` emits `import <local> from "<source>"`
    - `mode: "namespace"` emits `import * as <local> from "<source>"`

Rust: `InjectOptions { entries: Vec<InjectEntry> }`; `InjectSpecifier::{Named { imported, local }, Default { local }, Namespace { local }}`.

Injected imports run after every transform and before `define`.

## `define`

`define` replaces free identifiers and member chains with expression source strings. Quote string literals in the value.

```ts
transform({
    define: {
        __DEV__: "false",
        "process.env.NODE_ENV": '"production"',
        VERSION: 3,
    },
});
```

- a string value is expression source text; it passes through verbatim
- a number or boolean is stringified
- keys are processed in alphabetical order
- replacements run last, after `inject` and every transform
- invalid keys or values fail the compile
- in Rust, the option is `DefineOptions { entries: Vec<(String, String)> }`. The first entry for a duplicate key wins

## `oxc` Escape Hatch

`oxc` passes raw `OxcTransformOptions` to the backend.

- curated fields win over the raw base field by field
- `targets` wins over `oxc.env`
- the compile `cwd` overwrites `oxc.cwd`
- raw `oxc.jsx` fields alone do not enable JSX lowering
- in Rust, `helper_loader` wins over raw `oxc.helper_loader`; the raw helper state is ignored

Use `oxc` only for fields the curated options do not expose.

## Helper Loading in Rust

Rust: `HelperLoaderOptions { mode: Option<HelperLoaderMode>, module_name: Option<String> }`. The default mode is `Inline`; the default module name is `@oxc-project/runtime`.

| Mode       | Behavior                                            |
| ---------- | --------------------------------------------------- |
| `Inline`   | Insert helper bodies directly. This is the default. |
| `Runtime`  | Import helpers from the runtime package.            |
| `External` | Use the global `babelHelpers` object.               |

## Output File

`output.file` labels the generated code. The compiler does not write a file. When `output.file` is unset or empty, the transform plugin derives it from the input `file`:

| Input suffix                                     | Output suffix |
| ------------------------------------------------ | ------------- |
| `.ts`                                            | `.js`         |
| `.mts`                                           | `.mjs`        |
| `.cts`                                           | `.cjs`        |
| `.tsx` or `.jsx`, JSX preserved                  | `.jsx`        |
| `.tsx` or `.jsx`, JSX lowered                    | `.js`         |
| `.d.ts`, `.d.mts`, `.d.cts`, or any other suffix | unchanged     |

- a preserved `.tsx` still strips TypeScript, so its output name is `.jsx`. Derived names drop `?query` and `#hash` suffixes
- an explicit `language` selects the mapping: `tsx` maps `.ts`, `.js`, `.mjs`, and `.cjs` inputs to `.jsx` when JSX is preserved, and to `.js` when JSX is lowered. `js` and `dts` leave the name unchanged; unknown suffixes stay unchanged
- a non-empty `output.file` wins verbatim. Without the transform plugin, the resolved input `file` is used
- the plugin cannot see the AST language in its options hook. In `compileFromAst`, set `output.file` explicitly when the AST language differs from the grammar implied by `file` and `language`
- the plugin derives the name in a `Post` options hook. A later `Post` options hook that changes `file` or `language` must set `output.file` itself

## Pass Order

1. main oxc transformer
2. helper inlining when the resolved mode is `Inline`
3. `inject`
4. `define`

Source-map output delivery happens after the winning generate result, not inside the transform plugin.

## Common Mistakes

- treating `transform()` as a full syntax lowerer. It strips TypeScript, but `targets: []` keeps modern syntax
- expecting JSX to lower without `jsx.runtime`; raw `oxc.jsx` fields do not enable it
- forgetting that `useDefineForClassFields` defaults to `true`
- enabling `emitDecoratorMetadata` without `experimentalDecorators`
- passing unquoted strings to `define`, or expecting `define` to run before `inject`
- expecting `output.file` to write a file, or forgetting it in `compileFromAst` when the AST grammar differs from the file extension
- mixing camelCase and snake_case field names across the JavaScript and Rust APIs
