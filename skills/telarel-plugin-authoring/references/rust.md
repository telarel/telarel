# Rust Plugins

A Rust plugin implements the `Plugin` trait from the `telarel` crate. Public contract: [Rust plugin API](https://telarel.github.io/apis/plugin/rust).

## Imports

```rust
use std::borrow::Cow;
use std::future::Future;

use telarel::allocator::Allocator;
use telarel::ast::ast::{ImportDeclaration, Program};
use telarel::ast_visit::walk_mut;
use telarel::ast_visit::VisitMut;
use telarel::str::JSStr;
use telarel::{
    Ast, AstResult, CodeResult, CommonPluginContext, CompileError,
    CompileOptions, HookUsage, Language, OptionsArgs, OptionsReturn, ParseArgs,
    ParseReturn, Parser, ParserOptions, Plugin, PluginContext, PluginError,
    SourceType, TransformArgs, TransformOutput, TransformReturn, compile,
};
```

`ast_visit` is enabled by the `telarel` crate's default `ast_visit` feature.

## Plugin Shape

```rust
#[derive(Debug)]
struct MyPlugin;

impl Plugin for MyPlugin {
    fn name(&self) -> Cow<'static, str> {
        "my-plugin".into()
    }

    fn register_hook_usage(&self) -> HookUsage {
        HookUsage::Transform
    }

    fn transform<'a>(
        &'a self,
        _: &'a PluginContext<'_>,
        args: TransformArgs,
    ) -> impl Future<Output = TransformReturn> + Send + 'a {
        async move {
            let mut ast: Ast = args.ast;

            ast.with_mut(|_: &Allocator, program: &mut Program<'_>| {
                program.directives.clear();
            });

            Ok(Some(TransformOutput { ast }))
        }
    }
}
```

- `Plugin` requires `Any + Debug + Send + Sync + 'static`. Derive `Debug`
- `register_hook_usage` declares the gated hooks. `options`, `start`, and `end` run regardless of the declaration
- every hook has a default no-op implementation
- `Plugin::new_shared(MyPlugin)` wraps a sized plugin into `SharedPluginable`. The handle, not the name, is the `options` identity

## `HookUsage`

`HookUsage` is a bit flag set:

```rust
use telarel::HookUsage;

let usage: HookUsage = HookUsage::Prepare | HookUsage::End;
```

The flags are `Options`, `Start`, `Parse`, `Prepare`, `Transform`, `Finalize`, `Generate`, and `End`.

Only `Parse`, `Prepare`, `Transform`, `Finalize`, and `Generate` are gated by the declaration. If you implement `transform` but omit `HookUsage::Transform`, the driver never calls it.

## `Ast` and Mutation

`Ast` is an owned, self-referential AST. Cloning is O(1). `Ast::with_mut` is the only path to the arena; it copies on write when another handle is alive, so mutating a clone never mutates the original.

- `with_mut` can rewrite the tree but cannot change the source text
- `Program::source_text` must stay content-equal to `Ast::source`. Debug builds assert this
- to change the source text, build a new `Ast` with `Parser` or `Ast::from_source`, then return it from `transform`
- only `transform` carries mutations forward. Return `Ok(Some(TransformOutput { ast }))` to replace the carried AST. `Ok(None)` leaves the driver's AST untouched

## Recipes

Each snippet is a hook method inside an `impl Plugin`; add `name` and `register_hook_usage` as in Plugin Shape.

### Rewrite imports

Declare `HookUsage::Transform`; mutate through `Ast::with_mut` and return the AST:

```rust
fn transform<'a>(
    &'a self,
    _ctx: &'a PluginContext<'_>,
    args: TransformArgs,
) -> impl Future<Output = TransformReturn> + Send + 'a {
    async move {
        let mut ast: Ast = args.ast;

        ast.with_mut(|allocator: &Allocator, program: &mut Program<'_>| {
            let mut rewriter: ImportRewriter<'_> = ImportRewriter { allocator };
            walk_mut::walk_program(&mut rewriter, program);
        });

        Ok(Some(TransformOutput { ast }))
    }
}

struct ImportRewriter<'a> {
    allocator: &'a Allocator,
}

impl<'a> VisitMut<'a> for ImportRewriter<'a> {
    fn visit_import_declaration(
        &mut self,
        declaration: &mut ImportDeclaration<'a>,
    ) {
        if declaration.source.value == "react" {
            declaration.source.value =
                JSStr::from_str_in("preact/compat", &self.allocator);
        }
    }
}
```

### Adjust options

```rust
fn options<'a>(
    &'a self,
    _ctx: &'a CommonPluginContext,
    args: &'a OptionsArgs,
) -> impl Future<Output = OptionsReturn> + Send {
    async move {
        let mut next: OptionsArgs = args.clone();
        next.options.file = Some(String::from("custom.ts"));
        next.options.language = Some(Language::TS);
        next.plugins.push(Plugin::new_shared(TracePlugin));
        Ok(Some(next))
    }
}
```

- clone `args` so you keep the current plugin list and every unset field
- returning `Some(OptionsArgs { ... })` replaces the whole struct. Omitted fields take defaults, not previous values
- the returned plugin list replaces the settled list wholesale
- append a stable instance. The hook runs once per distinct instance, so creating the injected plugin inside the hook is fine. The depth guard applies when a hook creates a new instance on every pass

### Parse a custom input language (exclusive)

```rust
fn parse<'a>(
    &'a self,
    _ctx: &'a PluginContext<'_>,
    args: ParseArgs<'a>,
) -> impl Future<Output = ParseReturn> + Send + 'a {
    async move {
        if !args.code.starts_with("greet ") {
            return Ok(None);
        }

        let name: &str = args.code["greet ".len()..].trim();

        let message: String = format!("hi {name}");

        let js: String = format!("console.log({message:?});");

        let mut options: ParserOptions = args.options.clone();

        options.language = Language::JS;
        options.source_type = SourceType::Script;

        let parser: Parser = Parser::new(js, options);

        let result: AstResult = parser
            .parse()
            .map_err(|error: CompileError| PluginError::new(error.to_string()))?;

        Ok(Some(result))
    }
}
```

- `parse` replaces the builtin parser. Only declare it when the plugin owns the input language; every later `parse` hook is skipped when it returns an AST
- return `Ok(None)` for source that is not yours
- the returned AST's source is the translated JavaScript. If spans must point at the original source, build the `Ast` yourself with `Ast::from_source` or `Ast::try_from_source`
- `Parser::new` takes fully resolved `ParserOptions`. Clone `args.options` and override the grammar or module kind. Do not use `parse` for validation; use `prepare`

### Generate a custom output format (exclusive)

```rust
fn generate<'a>(
    &'a self,
    _ctx: &'a PluginContext<'_>,
    args: telarel::GenerateArgs<'a>,
) -> impl Future<Output = telarel::GenerateReturn> + Send + 'a {
    async move {
        let statements: usize = args.ast.program().body.len();

        Ok(Some(CodeResult {
            code: format!("{{\"statements\":{statements}}}"),
            map: None,
        }))
    }
}
```

- `generate` replaces the builtin generator. Only declare it when the plugin owns the output format; every later `generate` hook is skipped when it returns code
- return `Ok(None)` to defer. `args.options` carries an empty source-map request
- `args.ast` is borrowed read-only. A custom generator can return `map: None` when it does not produce a source map
- do not wrap or post-process builtin output here; post-process `CodeResult.code` at the call site

### Report an error with a source

```rust
fn prepare<'a>(
    &'a self,
    ctx: &'a PluginContext<'_>,
    args: telarel::PrepareArgs<'a>,
) -> impl Future<Output = telarel::NotifyReturn> + Send + 'a {
    async move {
        // find_debugger_span is a walker helper that returns the node span.
        if let Some(span) = find_debugger_span(args.ast.program()) {
            return Err(
                PluginError::new("debugger statements are not allowed")
                    .with_source_ast(ctx.module.file, args.ast, span),
            );
        }

        Ok(())
    }
}
```

`PluginError` surfaces at the entry point as a `CompileError` with the message and source unchanged.

## Ordering

Each hook has a `<hook>_meta` method returning `Option<PluginHookMeta>`:

```rust
use telarel::{PluginHookMeta, PluginOrder};

fn prepare_meta(&self) -> Option<PluginHookMeta> {
    Some(PluginHookMeta { order: Some(PluginOrder::Pre) })
}

fn finalize_meta(&self) -> Option<PluginHookMeta> {
    Some(PluginHookMeta { order: Some(PluginOrder::Post) })
}
```

Ordering is per hook, not per plugin. `pre` runs before every normal plugin, `post` after every normal plugin. The normal bucket keeps registration order.

## Passing Plugins

Wrap each plugin in `Plugin::new_shared` and pass the vector to the entry point:

```rust
let plugins: Vec<SharedPluginable> = vec![Plugin::new_shared(ImportRewrite)];
compile(code, CompileOptions::default(), plugins).await?;
```

## Testing

```rust
use telarel::{
    CodeResult, CompileOptions, Plugin, SharedPluginable, compile,
};

#[tokio::test(flavor = "current_thread")]
async fn rewrites_react_imports() {
    let plugins: Vec<SharedPluginable> =
        vec![Plugin::new_shared(ImportRewrite)];

    let result: CodeResult = compile(
        String::from("import { useState } from \"react\";"),
        CompileOptions::default(),
        plugins,
    )
    .await
    .expect("compile succeeds");

    assert!(result.code.contains("preact/compat"));
    assert!(!result.code.contains("from \"react\""));
}
```

Add `tokio` as a dev-dependency with `rt` and `macros` features. Test the smallest entry point that runs the hook, then the full `compile` path. Add one failure-path test for `PluginError` or malformed source.

## Common Mistakes

- implementing `transform` without `HookUsage::Transform`; the hook is skipped
- returning the `Ast` instead of `Ok(Some(TransformOutput { ast }))`
- calling `Ast::with_mut` and returning `Ok(None)`; the change is discarded
- replacing `Program::source_text` inside `with_mut`; build a new `Ast`
- returning a fresh `OptionsArgs` and dropping `args.plugins`
- forgetting to clone `args.options` in a parse or generate hook
- storing non-`Send` or non-`Sync` state in a plugin
- treating `Ast::map` as the generated output map
