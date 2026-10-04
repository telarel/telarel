pub mod pluginable;

use std::any::Any;
use std::borrow::Cow;
use std::fmt::Debug;
use std::future::Future;
use std::sync::Arc;

use telarel_common::HookUsage;

use crate::_types::context::{CommonPluginContext, PluginContext};
use crate::_types::hooks::compile_end::CompileEndArgs;
use crate::_types::hooks::compile_start::CompileStartArgs;
use crate::_types::hooks::finalize::{FinalizeArgs, FinalizeReturn};
use crate::_types::hooks::notify::NotifyReturn;
use crate::_types::hooks::options::{OptionsArgs, OptionsReturn};
use crate::_types::hooks::prepare::{PrepareArgs, PrepareReturn};
use crate::_types::hooks::transform::{TransformArgs, TransformReturn};
use crate::_types::order::PluginHookMeta;
use crate::plugin::pluginable::SharedPluginable;

/// The Rust plugin trait.
pub trait Plugin: Any + Debug + Send + Sync + 'static {
    /// Wrap a sized plugin into a shared, object-safe handle.
    fn new_shared(plugin: Self) -> SharedPluginable
    where
        Self: Sized,
    {
        Arc::new(plugin)
    }

    /// The plugin name.
    fn name(&self) -> Cow<'static, str>;

    /// Which hooks this plugin implements; hooks not declared are never called.
    fn register_hook_usage(&self) -> HookUsage;

    /// Run the `options` hook over the pipeline options args. Returning
    /// `Some` replaces the whole set (struct-level replace); `None` keeps
    /// the args unchanged.
    fn options<'a>(
        &'a self,
        _ctx: &'a CommonPluginContext,
        _args: &'a OptionsArgs,
    ) -> impl Future<Output = OptionsReturn> + Send {
        async { Ok(None) }
    }

    /// Ordering for the `options` hook; `None` = normal bucket.
    fn options_meta(&self) -> Option<PluginHookMeta> {
        None
    }

    /// Run the `compile_start` hook with the fully resolved, read-only
    /// options; notify-only.
    fn compile_start<'a>(
        &'a self,
        _ctx: &'a PluginContext<'_>,
        _args: &'a CompileStartArgs,
    ) -> impl Future<Output = NotifyReturn> + Send {
        async { Ok(()) }
    }

    /// Ordering for the `compile_start` hook; `None` = normal bucket.
    fn compile_start_meta(&self) -> Option<PluginHookMeta> {
        None
    }

    /// Run the `prepare` hook, before the transform chain. Returning `Some`
    /// replaces the source code; the carried map is incremental relative
    /// to the code this hook received.
    fn prepare<'a>(
        &'a self,
        _ctx: &'a PluginContext<'_>,
        _args: &'a PrepareArgs<'_>,
    ) -> impl Future<Output = PrepareReturn> + Send {
        async { Ok(None) }
    }

    /// Ordering for the `prepare` hook; `None` = normal bucket.
    fn prepare_meta(&self) -> Option<PluginHookMeta> {
        None
    }

    /// Run the `transform` hook; the AST is read-only. A plugin that changes
    /// the tree derives a new program rooted in `args.allocator` and returns
    /// it as `Some(TransformOutput)`; `None` leaves the carried AST untouched.
    fn transform<'a, 'ast: 'a>(
        &'a self,
        _ctx: &'a PluginContext<'a>,
        _args: TransformArgs<'ast>,
    ) -> impl Future<Output = TransformReturn<'ast>> + 'a {
        async { Ok(None) }
    }

    /// Ordering for the `transform` hook; `None` = normal bucket.
    fn transform_meta(&self) -> Option<PluginHookMeta> {
        None
    }

    /// Run the `finalize` hook, after the transform chain. Returning `Some`
    /// replaces the generated code; the carried map is incremental
    /// relative to the generated code this hook received.
    fn finalize<'a>(
        &'a self,
        _ctx: &'a PluginContext<'_>,
        _args: &'a FinalizeArgs<'_>,
    ) -> impl Future<Output = FinalizeReturn> + Send {
        async { Ok(None) }
    }

    /// Ordering for the `finalize` hook; `None` = normal bucket.
    fn finalize_meta(&self) -> Option<PluginHookMeta> {
        None
    }

    /// Run the `compile_end` hook with the final code, map, and error;
    /// notify-only. Core must invoke it on success and on every error path.
    fn compile_end<'a>(
        &'a self,
        _ctx: &'a PluginContext<'_>,
        _args: &'a CompileEndArgs,
    ) -> impl Future<Output = NotifyReturn> + Send {
        async { Ok(()) }
    }

    /// Ordering for the `compile_end` hook; `None` = normal bucket.
    fn compile_end_meta(&self) -> Option<PluginHookMeta> {
        None
    }
}

#[cfg(test)]
#[allow(clippy::manual_async_fn)]
mod tests {
    use std::borrow::Cow;
    use std::future::Future;
    use std::sync::Arc;
    use std::sync::Mutex;

    use oxc::allocator::Allocator;
    use oxc::allocator::CloneIn;
    use oxc::ast::ast::{Directive, Program, StringLiteral};
    use oxc::ast::builder::AstBuilder;
    use oxc::span::SPAN;
    use telarel_common::{
        CompileContext, HookUsage, Language, ParseOptions, ParseResult,
        SourceType, parse,
    };

    use crate::_types::context::{
        CommonPluginContext, ModuleInfo, PluginContext,
    };
    use crate::_types::hooks::compile_end::CompileEndArgs;
    use crate::_types::hooks::compile_start::CompileStartArgs;
    use crate::_types::hooks::finalize::FinalizeOutput;
    use crate::_types::hooks::prepare::{PrepareArgs, PrepareOutput};
    use crate::_types::hooks::transform::{TransformArgs, TransformOutput};
    use crate::plugin::pluginable::SharedPluginable;

    use super::*;

    #[derive(Debug)]
    struct ProbePlugin;

    impl Plugin for ProbePlugin {
        fn name(&self) -> Cow<'static, str> {
            "probe".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Prepare
        }
    }

    #[derive(Debug)]
    struct OptionsReplacePlugin {
        file: &'static str,
    }

    impl Plugin for OptionsReplacePlugin {
        fn name(&self) -> Cow<'static, str> {
            "options-replace".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Options
        }

        fn options<'a>(
            &'a self,
            _ctx: &'a CommonPluginContext,
            args: &'a OptionsArgs,
        ) -> impl Future<Output = OptionsReturn> + Send {
            async move {
                let mut next: OptionsArgs = args.clone();

                next.options.file = self.file.to_string();

                Ok(Some(next))
            }
        }
    }

    #[derive(Debug)]
    struct PrepareReplacePlugin;

    impl Plugin for PrepareReplacePlugin {
        fn name(&self) -> Cow<'static, str> {
            "prepare-replace".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Prepare
        }

        fn prepare<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            _args: &'a PrepareArgs<'_>,
        ) -> impl Future<Output = PrepareReturn> + Send {
            async {
                Ok(Some(PrepareOutput {
                    code: String::from("let b;"),
                    map: None,
                }))
            }
        }
    }

    #[derive(Debug)]
    struct FinalizeReplacePlugin;

    impl Plugin for FinalizeReplacePlugin {
        fn name(&self) -> Cow<'static, str> {
            "finalize-replace".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Finalize
        }

        fn finalize<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            _args: &'a FinalizeArgs<'_>,
        ) -> impl Future<Output = FinalizeReturn> + Send {
            async {
                Ok(Some(FinalizeOutput {
                    code: String::from("let c;"),
                    map: None,
                }))
            }
        }
    }

    #[derive(Debug)]
    struct CloneMutatePlugin;

    impl Plugin for CloneMutatePlugin {
        fn name(&self) -> Cow<'static, str> {
            "clone-mutate".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Transform
        }

        fn transform<'a, 'ast: 'a>(
            &'a self,
            _ctx: &'a PluginContext<'a>,
            args: TransformArgs<'ast>,
        ) -> impl Future<Output = TransformReturn<'ast>> + 'a {
            async move {
                let mut working: Program<'ast> =
                    (*args.ast).clone_in(args.allocator);

                let builder: AstBuilder<'ast> = AstBuilder::new(args.allocator);

                let string_literal: StringLiteral<'ast> =
                    StringLiteral::new(SPAN, "\"mark\";", None, &builder);

                let directive: Directive<'ast> =
                    Directive::new(SPAN, string_literal, "mark", &builder);

                working.directives.push(directive);

                let rooted: &'ast Program<'ast> = args.allocator.alloc(working);

                Ok(Some(TransformOutput { ast: rooted }))
            }
        }
    }

    #[derive(Debug)]
    struct ReplaceRootPlugin;

    impl Plugin for ReplaceRootPlugin {
        fn name(&self) -> Cow<'static, str> {
            "replace-root".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Transform
        }

        fn transform<'a, 'ast: 'a>(
            &'a self,
            _ctx: &'a PluginContext<'a>,
            args: TransformArgs<'ast>,
        ) -> impl Future<Output = TransformReturn<'ast>> + 'a {
            async move {
                let code: &'ast str = args.allocator.alloc_str("\"replaced\";");

                let file: &'ast str =
                    args.allocator.alloc_str(_ctx.module.file);

                let ctx: CompileContext<'_> =
                    CompileContext::new(_ctx.cwd, file, code);

                let parsed: ParseResult<'ast> = parse(ParseOptions {
                    context: &ctx,
                    allocator: args.allocator,
                    file,
                    code,
                    language: None,
                    source_type: None,
                })?;

                let fresh: &'ast Program<'ast> =
                    args.allocator.alloc(parsed.program);

                Ok(Some(TransformOutput { ast: fresh }))
            }
        }
    }

    #[derive(Debug)]
    struct FailingOptionsPlugin;

    impl Plugin for FailingOptionsPlugin {
        fn name(&self) -> Cow<'static, str> {
            "fail-options".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Options
        }

        fn options<'a>(
            &'a self,
            _ctx: &'a CommonPluginContext,
            _args: &'a OptionsArgs,
        ) -> impl Future<Output = OptionsReturn> + Send {
            async { Err(anyhow::anyhow!("boom")) }
        }
    }

    type EndedLog = Vec<(String, Option<String>)>;

    #[derive(Debug)]
    struct NotifyPlugin {
        started: Arc<Mutex<Vec<String>>>,
        ended: Arc<Mutex<EndedLog>>,
    }

    impl Plugin for NotifyPlugin {
        fn name(&self) -> Cow<'static, str> {
            "notify".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::CompileStart | HookUsage::CompileEnd
        }

        fn compile_start<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            args: &'a CompileStartArgs,
        ) -> impl Future<Output = NotifyReturn> + Send {
            async move {
                self.started.lock().unwrap().push(args.options.cwd.clone());

                Ok(())
            }
        }

        fn compile_end<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            args: &'a CompileEndArgs,
        ) -> impl Future<Output = NotifyReturn> + Send {
            async move {
                self.ended
                    .lock()
                    .unwrap()
                    .push((args.code.clone(), args.err.clone()));

                Ok(())
            }
        }
    }

    fn make_context<'a>(code: &'a str) -> PluginContext<'a> {
        let module: ModuleInfo<'a> = ModuleInfo {
            file: "a.ts",
            code,
            language: Language::TS,
            source_type: SourceType::Module,
        };

        PluginContext::new("/repo", module)
    }

    fn make_args(plugins: Vec<SharedPluginable>) -> OptionsArgs {
        OptionsArgs {
            options: telarel_common::CompileOptions {
                file: "a.ts".into(),
                code: "console.log(1);".into(),
                ..telarel_common::CompileOptions::default()
            },
            plugins,
        }
    }

    #[test]
    fn test_new_shared_wraps_plugin() {
        let shared: SharedPluginable = Plugin::new_shared(ProbePlugin);

        assert_eq!(shared.call_name(), "probe");
        assert!(shared.call_register_hook_usage().contains(HookUsage::Prepare));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_default_hooks_are_noops() {
        let common: CommonPluginContext = CommonPluginContext::default();

        let ctx: PluginContext<'_> = make_context("console.log(1);");

        let shared: SharedPluginable = Plugin::new_shared(ProbePlugin);

        let args: OptionsArgs = make_args(vec![]);

        assert!(shared.call_options(&common, &args).await.unwrap().is_none());
        assert!(
            shared
                .call_prepare(&ctx, &PrepareArgs { code: "let a;" })
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            shared
                .call_finalize(
                    &ctx,
                    &crate::_types::hooks::finalize::FinalizeArgs {
                        code: "let a;"
                    }
                )
                .await
                .unwrap()
                .is_none()
        );
        shared
            .call_compile_start(
                &ctx,
                &CompileStartArgs {
                    options: telarel_common::ResolvedOptions {
                        cwd: String::from("/repo"),
                        file: String::from("a.ts"),
                        code: String::from("let a;"),
                        language: Language::TS,
                        source_type: SourceType::Module,
                        plugins: vec![],
                    },
                },
            )
            .await
            .unwrap();
        shared
            .call_compile_end(
                &ctx,
                &CompileEndArgs {
                    code: String::from("let a;"),
                    map: None,
                    err: None,
                },
            )
            .await
            .unwrap();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_options_none_keeps_args() {
        let common: CommonPluginContext = CommonPluginContext::default();

        let args: OptionsArgs = make_args(vec![]);

        let shared: SharedPluginable = Plugin::new_shared(ProbePlugin);

        let result: Option<OptionsArgs> =
            shared.call_options(&common, &args).await.unwrap();

        assert!(result.is_none());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_options_some_replaces_args() {
        let common: CommonPluginContext = CommonPluginContext::default();

        let args: OptionsArgs = make_args(vec![]);

        let shared: SharedPluginable =
            Plugin::new_shared(OptionsReplacePlugin { file: "b.ts" });

        let next: OptionsArgs =
            shared.call_options(&common, &args).await.unwrap().unwrap();

        assert_eq!(next.options.file, "b.ts");
        assert_eq!(next.options.code, "console.log(1);");
        assert!(next.plugins.is_empty());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_options_error_propagates() {
        let common: CommonPluginContext = CommonPluginContext::default();

        let args: OptionsArgs = make_args(vec![]);

        let shared: SharedPluginable = Plugin::new_shared(FailingOptionsPlugin);

        let err: anyhow::Error =
            shared.call_options(&common, &args).await.unwrap_err();

        assert!(format!("{err:#}").contains("boom"), "{err:#}");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_prepare_and_finalize_return_outputs() {
        let ctx: PluginContext<'_> = make_context("console.log(1);");

        let prepare: SharedPluginable =
            Plugin::new_shared(PrepareReplacePlugin);

        let prepare_output: PrepareOutput = prepare
            .call_prepare(&ctx, &PrepareArgs { code: "console.log(1);" })
            .await
            .unwrap()
            .unwrap();

        assert_eq!(prepare_output.code, "let b;");
        assert!(prepare_output.map.is_none());

        let finalize: SharedPluginable =
            Plugin::new_shared(FinalizeReplacePlugin);

        let finalize_output: FinalizeOutput = finalize
            .call_finalize(
                &ctx,
                &crate::_types::hooks::finalize::FinalizeArgs {
                    code: "let b;",
                },
            )
            .await
            .unwrap()
            .unwrap();

        assert_eq!(finalize_output.code, "let c;");
        assert!(finalize_output.map.is_none());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_transform_none_keeps_original_program() {
        let allocator: Allocator = Allocator::default();

        let ctx: PluginContext<'_> = make_context("console.log(1);");

        let parsed: ParseResult<'_> = parse(ParseOptions {
            context: &telarel_common::CompileContext::new(
                "/repo",
                "a.ts",
                "console.log(1);",
            ),
            allocator: &allocator,
            file: "a.ts",
            code: "console.log(1);",
            language: None,
            source_type: None,
        })
        .unwrap();

        let program: Program<'_> = parsed.program;

        let args: TransformArgs<'_> =
            TransformArgs { allocator: &allocator, ast: &program };

        let shared: SharedPluginable = Plugin::new_shared(ProbePlugin);

        let result: Option<TransformOutput<'_>> =
            shared.call_transform(&ctx, args).await.unwrap();

        assert!(result.is_none());

        let out: String = oxc::codegen::Codegen::new().build(&program).code;

        assert!(out.contains("console.log"), "{out}");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_transform_clone_mutation_is_returned() {
        let allocator: Allocator = Allocator::default();

        let ctx: PluginContext<'_> = make_context("console.log(1);");

        let parsed: ParseResult<'_> = parse(ParseOptions {
            context: &telarel_common::CompileContext::new(
                "/repo",
                "a.ts",
                "console.log(1);",
            ),
            allocator: &allocator,
            file: "a.ts",
            code: "console.log(1);",
            language: None,
            source_type: None,
        })
        .unwrap();

        let program: Program<'_> = parsed.program;

        let args: TransformArgs<'_> =
            TransformArgs { allocator: &allocator, ast: &program };

        let shared: SharedPluginable = Plugin::new_shared(CloneMutatePlugin);

        let result: Option<TransformOutput<'_>> =
            shared.call_transform(&ctx, args).await.unwrap();

        let output: TransformOutput<'_> =
            result.expect("clone-mutate must return Some");

        let out: String = oxc::codegen::Codegen::new().build(output.ast).code;

        assert!(out.contains("mark"), "{out}");

        let original: String =
            oxc::codegen::Codegen::new().build(&program).code;

        assert!(
            !original.contains("mark"),
            "the input program must stay untouched: {original}"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_transform_some_returns_replaced_program() {
        let allocator: Allocator = Allocator::default();

        let ctx: PluginContext<'_> = make_context("console.log(1);");

        let parsed: ParseResult<'_> = parse(ParseOptions {
            context: &telarel_common::CompileContext::new(
                "/repo",
                "a.ts",
                "console.log(1);",
            ),
            allocator: &allocator,
            file: "a.ts",
            code: "console.log(1);",
            language: None,
            source_type: None,
        })
        .unwrap();

        let program: Program<'_> = parsed.program;

        let args: TransformArgs<'_> =
            TransformArgs { allocator: &allocator, ast: &program };

        let shared: SharedPluginable = Plugin::new_shared(ReplaceRootPlugin);

        let output: TransformOutput<'_> =
            shared.call_transform(&ctx, args).await.unwrap().unwrap();

        let out: String = oxc::codegen::Codegen::new().build(output.ast).code;

        assert!(out.contains("replaced"), "{out}");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_start_and_end_receive_args() {
        let started: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let ended: Arc<Mutex<EndedLog>> = Arc::new(Mutex::new(Vec::new()));

        let ctx: PluginContext<'_> = make_context("console.log(1);");

        let shared: SharedPluginable = Plugin::new_shared(NotifyPlugin {
            started: Arc::clone(&started),
            ended: Arc::clone(&ended),
        });

        shared
            .call_compile_start(
                &ctx,
                &CompileStartArgs {
                    options: telarel_common::ResolvedOptions {
                        cwd: String::from("/resolved"),
                        file: String::from("a.ts"),
                        code: String::from("let a;"),
                        language: Language::TS,
                        source_type: SourceType::Module,
                        plugins: vec![String::from("probe")],
                    },
                },
            )
            .await
            .unwrap();

        shared
            .call_compile_end(
                &ctx,
                &CompileEndArgs {
                    code: String::from("let b;"),
                    map: None,
                    err: Some(String::from("boom")),
                },
            )
            .await
            .unwrap();

        assert_eq!(*started.lock().unwrap(), vec![String::from("/resolved")]);

        assert_eq!(
            *ended.lock().unwrap(),
            vec![(String::from("let b;"), Some(String::from("boom")))]
        );
    }
}
