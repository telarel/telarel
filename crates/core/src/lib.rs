//! # Telarel Core
//!
//! A pipeline orchestrator for the compiler.
//!
//! This crate drives the per-file compile pipeline: it runs plugin hooks,
//! parses the source, applies transforms, and produces code with a map.

use oxc::allocator::Allocator;
use oxc::ast::ast::Program;
use oxc_sourcemap::SourceMap as OxcSourceMap;
use oxc_sourcemap::SourceMapBuilder;

use telarel_common::{
    CompileContext, CompileError, HookUsage, ParseOptions, parse,
};
use telarel_plugin::__internal::PluginDriver;
use telarel_plugin::{PostArgs, PreArgs, SharedPluginable, TransformArgs};

pub use telarel_common::CompileOptions;

/// An owned source map produced by a compile run.
pub type SourceMap = OxcSourceMap<'static>;

/// Strip a trailing `\n`, then a trailing `\r`, so `\n`, `\r\n`, and
/// lone `\r` terminators all disappear, but still keeping interior
/// line endings survive.
///
/// The input is owned, so the trailing terminator is removed in place
/// instead of copying the whole string into a fresh `String`.
fn strip_trailing_newline(mut source: String) -> String {
    let new_len: usize =
        source.trim_end_matches('\n').trim_end_matches('\r').len();

    source.truncate(new_len);

    source
}

/// Build a per-line identity map: each generated line maps to the same
/// source line at column 0.
fn identity_map(
    file: &str,
    code: &str,
) -> SourceMap {
    let mut builder: SourceMapBuilder<'_> = SourceMapBuilder::default();

    builder.set_file(file);

    let source_id: u32 = builder.add_source_and_content(file, code);

    let line_count: u32 =
        u32::try_from(code.lines().count().max(1)).unwrap_or(u32::MAX);

    for line in 0..line_count {
        builder.add_token(line, 0, line, 0, Some(source_id), None);
    }

    builder.into_owned_sourcemap().into_inner()
}

/// Result of a successful compile.
#[derive(Debug, Clone)]
pub struct CompileOutput {
    /// Generated code.
    pub code: String,
    /// Source map.
    pub map: SourceMap,
}

/// Run the per-file pipeline over `options` with `plugins`.
pub async fn compile(
    options: CompileOptions,
    plugins: Vec<SharedPluginable>,
) -> Result<CompileOutput, CompileError> {
    let driver: PluginDriver = PluginDriver::new(plugins);

    let mut resolved: CompileOptions = options;

    resolved.cwd.get_or_insert_with(|| {
        std::env::current_dir()
            .map(|p: std::path::PathBuf| p.to_string_lossy().into_owned())
            .unwrap_or_else(|_| String::from("/"))
    });

    driver.options(&mut resolved).await.map_err(|error| {
        CompileError::from_message(&format!("options hook: {error:#}"))
    })?;

    let ctx: CompileContext<'_> = CompileContext::new(
        resolved.cwd.as_deref().unwrap_or("/"),
        &resolved.file,
        &resolved.code,
    );

    driver
        .pre(&ctx, &PreArgs { file: &resolved.file, code: &resolved.code })
        .await
        .map_err(|error| {
            CompileError::from_message(&format!("pre hook: {error:#}"))
        })?;

    let (code, map): (String, SourceMap) =
        if driver.usage().contains(HookUsage::Transform) {
            // Parse ONCE before the transform chain. Root the resolved source in
            // the allocator so the parsed program borrows from the same
            // allocation.
            let allocator: Allocator = Allocator::default();

            let source: &'_ str = allocator.alloc_str(&resolved.code);

            let parse_options: ParseOptions<'_, '_> = ParseOptions {
                context: &ctx,
                allocator: &allocator,
                file: &resolved.file,
                code: source,
                language: resolved.language,
                source_type: resolved.source_type,
            };

            let mut original: Program<'_> = parse(parse_options)?.program;

            {
                let mut transform_args: TransformArgs<'_, '_> = TransformArgs {
                    allocator: &allocator,
                    file: &resolved.file,
                    program: &mut original,
                };

                driver.transform(&ctx, &mut transform_args).await.map_err(
                    |error| {
                        CompileError::from_message(&format!(
                            "transform hook: {error:#}"
                        ))
                    },
                )?;
            }

            // Plugins mutated the program in place; codegen borrows the final
            // tree and never copies the program.
            let result: telarel_common::CodegenResult<'_> =
                telarel_common::codegen(telarel_common::CodegenOptions {
                    file: &resolved.file,
                    program: &original,
                });

            let code: String = strip_trailing_newline(result.code);

            let map: SourceMap = result.map.into_owned();

            (code, map)
        } else {
            // No plugin uses `transform`: skip parse and codegen entirely and
            // pass the source through with a per-line identity map. A clone is
            // required here: `ctx` borrows `resolved.code` until the `post` hook
            // below, so the strip cannot consume `resolved.code` in place.
            let code: String = strip_trailing_newline(resolved.code.clone());

            let map: SourceMap = identity_map(&resolved.file, &resolved.code);

            (code, map)
        };

    driver
        .post(&ctx, &PostArgs { file: &resolved.file, code: &code })
        .await
        .map_err(|error| {
            CompileError::from_message(&format!("post hook: {error:#}"))
        })?;

    Ok(CompileOutput { code, map })
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use telarel_common::CompileContext;
    use telarel_common::CompileOptions;
    use telarel_plugin::{
        Plugin, SharedPluginable, TransformArgs, TransformReturn,
    };

    use oxc::ast_visit::VisitMut;
    use oxc::ast_visit::walk_mut;

    use oxc::ast::ast::{Directive, IdentifierReference, StringLiteral};
    use oxc::ast::builder::AstBuilder;
    use oxc::span::SPAN;
    use oxc::str::Ident;

    use super::*;

    #[derive(Debug)]
    struct NoopPlugin;

    impl Plugin for NoopPlugin {
        fn name(&self) -> Cow<'static, str> {
            "noop".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Pre
        }
    }

    const TARGET: &str = "console";

    const REPLACEMENT: &str = "consolex";

    const DIRECTIVE: &str = "x-telarel-chain";

    #[derive(Debug)]
    struct RenameCalleePlugin;

    impl Plugin for RenameCalleePlugin {
        fn name(&self) -> Cow<'static, str> {
            "rename-callee".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Transform
        }

        // Mutate every `console` IdentifierReference to `consolex` in place
        // (VisitMut walk over the arena-allocated program).
        async fn transform<'a, 'ast>(
            &'a self,
            _ctx: &'a CompileContext<'a>,
            args: TransformArgs<'a, 'ast>,
        ) -> TransformReturn {
            let mut renamer: Renamer<'_> =
                Renamer { allocator: args.allocator };

            walk_mut::walk_program(&mut renamer, args.program);

            Ok(())
        }
    }

    struct Renamer<'x> {
        allocator: &'x Allocator,
    }

    impl<'x> VisitMut<'x> for Renamer<'x> {
        fn visit_identifier_reference(
            &mut self,
            ident: &mut IdentifierReference<'x>,
        ) {
            if ident.name.as_str() == TARGET {
                ident.name = Ident::from_str_in(REPLACEMENT, &self.allocator);
            }
        }
    }

    #[derive(Debug)]
    struct AppendDirectivePlugin;

    impl Plugin for AppendDirectivePlugin {
        fn name(&self) -> Cow<'static, str> {
            "append-directive".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Transform
        }

        // Mutate the program in place: append one extra directive.
        async fn transform<'a, 'ast>(
            &'a self,
            _ctx: &'a CompileContext<'a>,
            args: TransformArgs<'a, 'ast>,
        ) -> TransformReturn {
            let builder: AstBuilder<'ast> = AstBuilder::new(args.allocator);

            let string_literal: StringLiteral<'ast> =
                StringLiteral::new(SPAN, DIRECTIVE, None, &builder);

            let directive: Directive<'ast> =
                Directive::new(SPAN, string_literal, DIRECTIVE, &builder);

            args.program.directives.push(directive);

            Ok(())
        }
    }

    #[derive(Debug)]
    struct NoopTransformPlugin;

    impl Plugin for NoopTransformPlugin {
        fn name(&self) -> Cow<'static, str> {
            "noop-transform".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Transform
        }

        // A transform hook that never mutates the program: the current
        // program stays in place for the rest of the chain.
        async fn transform<'a, 'ast>(
            &'a self,
            _ctx: &'a CompileContext<'a>,
            _args: TransformArgs<'a, 'ast>,
        ) -> TransformReturn {
            Ok(())
        }
    }

    #[derive(Debug)]
    struct RewriteCodeOptionsPlugin;

    impl Plugin for RewriteCodeOptionsPlugin {
        fn name(&self) -> Cow<'static, str> {
            "rewrite-code-options".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Options
        }

        async fn options(
            &self,
            options: &mut CompileOptions,
        ) -> anyhow::Result<()> {
            options.code = "const rewritten = 7;".to_string();
            Ok(())
        }
    }

    #[derive(Debug)]
    struct ObserveCwdOptionsPlugin {
        observed: std::sync::Arc<std::sync::Mutex<Vec<Option<String>>>>,
    }

    impl Plugin for ObserveCwdOptionsPlugin {
        fn name(&self) -> Cow<'static, str> {
            "observe-cwd-options".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Options
        }

        async fn options(
            &self,
            options: &mut CompileOptions,
        ) -> anyhow::Result<()> {
            self.observed.lock().unwrap().push(options.cwd.clone());

            Ok(())
        }
    }

    #[derive(Debug)]
    struct FailingPrePlugin;

    impl Plugin for FailingPrePlugin {
        fn name(&self) -> Cow<'static, str> {
            "failing-pre".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Pre
        }

        async fn pre(
            &self,
            _ctx: &'_ CompileContext<'_>,
            _args: &'_ telarel_plugin::PreArgs<'_>,
        ) -> anyhow::Result<()> {
            Err(anyhow::anyhow!("stage boom"))
        }
    }

    #[derive(Debug)]
    struct FailingPostPlugin;

    impl Plugin for FailingPostPlugin {
        fn name(&self) -> Cow<'static, str> {
            "failing-post".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Post
        }

        async fn post(
            &self,
            _ctx: &'_ CompileContext<'_>,
            _args: &'_ telarel_plugin::PostArgs<'_>,
        ) -> anyhow::Result<()> {
            Err(anyhow::anyhow!("stage boom"))
        }
    }

    fn options() -> CompileOptions {
        CompileOptions {
            file: "index.ts".to_string(),
            code: "const a = 1;".to_string(),
            ..Default::default()
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_no_plugins_codegens_original() {
        let out: crate::CompileOutput =
            compile(options(), vec![]).await.unwrap();

        assert_eq!(out.code, "const a = 1;");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_noop_plugin_leaves_output_unchanged() {
        let opts: CompileOptions = CompileOptions {
            file: "index.ts".to_string(),
            code: "console.log(1);".to_string(),
            ..Default::default()
        };

        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(NoopPlugin)];

        let out = compile(opts, plugins).await.unwrap();

        assert_eq!(out.code, "console.log(1);");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_strips_trailing_newline() {
        let opts: CompileOptions = CompileOptions {
            file: "index.ts".to_string(),
            code: "const a = 1;\n".to_string(),
            ..Default::default()
        };

        let out: crate::CompileOutput = compile(opts, vec![]).await.unwrap();

        assert_eq!(out.code, "const a = 1;");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_strips_crlf_trailing_newline() {
        // CRLF sources must not leave a stray `\r` after the `\n` strip.
        let opts: CompileOptions = CompileOptions {
            file: "index.ts".to_string(),
            code: "const a = 1;\r\n".to_string(),
            ..Default::default()
        };

        let out: crate::CompileOutput = compile(opts, vec![]).await.unwrap();

        assert_eq!(out.code, "const a = 1;");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_preserves_interior_crlf() {
        // Only the trailing terminator is stripped; interior CRLF is intact.
        let opts: CompileOptions = CompileOptions {
            file: "index.ts".to_string(),
            code: "const a = 1;\r\nconst b = 2;".to_string(),
            ..Default::default()
        };

        let out: crate::CompileOutput = compile(opts, vec![]).await.unwrap();

        assert_eq!(out.code, "const a = 1;\r\nconst b = 2;");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_strips_lone_cr_trailing_newline() {
        // A lone `\r` terminator is also stripped, matching the documented
        // behavior of the trim (`\n`, `\r\n`, and lone `\r`).
        let opts: CompileOptions = CompileOptions {
            file: "index.ts".to_string(),
            code: "const a = 1;\r".to_string(),
            ..Default::default()
        };

        let out: crate::CompileOutput = compile(opts, vec![]).await.unwrap();

        assert_eq!(out.code, "const a = 1;");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_no_plugins_sourcemap_sources() {
        let out: crate::CompileOutput =
            compile(options(), vec![]).await.unwrap();

        assert_eq!(out.map.get_source(0), Some("index.ts"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_transform_plugin_replaces() {
        // RenameCalleePlugin over `console.log(1);` must yield `consolex`.
        let opts: CompileOptions = CompileOptions {
            file: "index.ts".to_string(),
            code: "console.log(1);".to_string(),
            ..Default::default()
        };

        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(RenameCalleePlugin)];

        let out = compile(opts, plugins).await.unwrap();

        assert!(out.code.contains("consolex"), "{}", out.code);

        let map: SourceMap = out.map;

        assert!(
            map.get_tokens().next().is_some(),
            "mappings must be non-empty"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_transform_sourcemap_points_at_original() {
        let opts: CompileOptions = CompileOptions {
            file: "index.ts".to_string(),
            code: "console.log(1);".to_string(),
            ..Default::default()
        };

        let out = compile(opts, vec![Plugin::new_shared(RenameCalleePlugin)])
            .await
            .unwrap();

        let map: SourceMap = out.map;

        assert_eq!(map.get_source(0), Some("index.ts"));
        assert!(
            map.get_tokens().next().is_some(),
            "mappings must be non-empty"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_invalid_code_errors_with_transform_plugin() {
        let opts: CompileOptions = CompileOptions {
            file: "index.ts".to_string(),
            code: "const = ;".to_string(),
            ..Default::default()
        };

        let error: CompileError =
            compile(opts, vec![Plugin::new_shared(RenameCalleePlugin)])
                .await
                .unwrap_err();

        assert!(error.to_string().contains("index.ts"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_no_transform_usage_skips_parse() {
        // Invalid syntax passes through unparsed when no plugin uses
        // `transform` — the skip path never parses.
        let opts: CompileOptions = CompileOptions {
            file: "index.ts".to_string(),
            code: "const = ;".to_string(),
            ..Default::default()
        };

        let out: crate::CompileOutput = compile(opts, vec![]).await.unwrap();

        assert_eq!(out.code, "const = ;");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_no_transform_usage_identity_map() {
        let opts: CompileOptions = CompileOptions {
            file: "index.ts".to_string(),
            code: "const a = 1;\nconst b = 2;\n".to_string(),
            ..Default::default()
        };

        let out: crate::CompileOutput = compile(opts, vec![]).await.unwrap();

        assert_eq!(out.code, "const a = 1;\nconst b = 2;");
        assert_eq!(out.map.get_source(0), Some("index.ts"));

        let token: oxc_sourcemap::Token =
            out.map.get_token(1).expect("one token per line");

        assert_eq!(token.get_dst_line(), 1);
        assert_eq!(token.get_src_line(), 1);
        assert_eq!(token.get_dst_col(), 0);
        assert_eq!(token.get_src_col(), 0);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_noop_pre_plugin_does_not_parse() {
        // A plugin declaring only PRE must not force the parse path; output
        // stays verbatim (proves NoopPlugin's PRE declaration doesn't leak
        // into the transform decision).
        let opts: CompileOptions = CompileOptions {
            file: "index.ts".to_string(),
            code: "console.log(1);\n".to_string(),
            ..Default::default()
        };

        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(NoopPlugin)];

        let out = compile(opts, plugins).await.unwrap();

        assert_eq!(out.code, "console.log(1);");
        assert_eq!(out.map.get_source(0), Some("index.ts"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_options_hook_rewrites_code() {
        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(RewriteCodeOptionsPlugin)];

        let out: crate::CompileOutput =
            compile(options(), plugins).await.unwrap();

        assert!(out.code.contains("rewritten"), "{}", out.code);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_pre_hook_error_aborts() {
        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(FailingPrePlugin)];

        let error: CompileError =
            compile(options(), plugins).await.unwrap_err();

        let message: String = error.to_string();

        assert!(message.contains("pre hook"), "{}", message);
        assert!(message.contains("stage boom"), "{}", message);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_output_map_is_alias() {
        // `SourceMap` in `CompileOutput` must be the owned-map alias: assign it
        // to an explicitly-aliased variable to prove the types agree.
        let opts: CompileOptions = CompileOptions {
            file: "index.ts".to_string(),
            code: "const a = 1;".to_string(),
            ..Default::default()
        };

        let out: crate::CompileOutput = compile(opts, vec![]).await.unwrap();

        let map: SourceMap = out.map;

        assert_eq!(map.get_source(0), Some("index.ts"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_post_hook_error_aborts() {
        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(FailingPostPlugin)];

        let error: CompileError =
            compile(options(), plugins).await.unwrap_err();

        let message: String = error.to_string();

        assert!(message.contains("post hook"), "{}", message);
        assert!(message.contains("stage boom"), "{}", message);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_transform_chain_multiple_replacements() {
        // Two replacing plugins in sequence: the rename must survive into the
        // second plugin and both transformations must appear in the final
        // code (proves the chain carries each replacement by reference).
        let opts: CompileOptions = CompileOptions {
            file: "index.ts".to_string(),
            code: "console.log(1);".to_string(),
            ..Default::default()
        };

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(RenameCalleePlugin),
            Plugin::new_shared(AppendDirectivePlugin),
        ];

        let out = compile(opts, plugins).await.unwrap();

        assert!(out.code.contains("consolex"), "{}", out.code);
        assert!(out.code.contains("x-telarel-chain"), "{}", out.code);
        assert_eq!(out.map.get_source(0), Some("index.ts"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_transform_noop_plugin_keeps_original() {
        // A transform plugin that keeps the parsed original in place must
        // leave it untouched for codegen: the code is unchanged verbatim.
        let opts: CompileOptions = CompileOptions {
            file: "index.ts".to_string(),
            code: "console.log(1);".to_string(),
            ..Default::default()
        };

        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(NoopTransformPlugin)];

        let out = compile(opts, plugins).await.unwrap();

        assert!(out.code.contains("console.log(1);"), "{}", out.code);
        assert_eq!(out.map.get_source(0), Some("index.ts"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_transform_mixed_chain_noop_then_replace() {
        // Noop, replace, noop: only the replacing plugin's mutation must
        // reach codegen, and the surrounding noop plugins must not reset
        // the tree to the original.
        let opts: CompileOptions = CompileOptions {
            file: "index.ts".to_string(),
            code: "console.log(1);".to_string(),
            ..Default::default()
        };

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(NoopTransformPlugin),
            Plugin::new_shared(RenameCalleePlugin),
            Plugin::new_shared(NoopTransformPlugin),
        ];

        let out = compile(opts, plugins).await.unwrap();

        assert!(out.code.contains("consolex"), "{}", out.code);
        assert_eq!(out.map.get_source(0), Some("index.ts"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_omitted_cwd_resolves_to_process_cwd() {
        let observed: std::sync::Arc<std::sync::Mutex<Vec<Option<String>>>> =
            std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(ObserveCwdOptionsPlugin {
                observed: std::sync::Arc::clone(&observed),
            })];

        let opts: CompileOptions = CompileOptions {
            file: "index.ts".to_string(),
            code: "const a = 1;".to_string(),
            ..Default::default()
        };

        compile(opts, plugins).await.unwrap();

        let expected: Option<String> = Some(
            std::env::current_dir()
                .map(|p: std::path::PathBuf| p.to_string_lossy().into_owned())
                .unwrap_or_else(|_| String::from("/")),
        );

        let recorded: Vec<Option<String>> = observed.lock().unwrap().clone();

        assert_eq!(recorded, vec![expected]);
    }
}
