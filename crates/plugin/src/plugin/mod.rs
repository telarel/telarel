pub mod pluginable;

use std::any::Any;
use std::borrow::Cow;
use std::fmt::Debug;
use std::future::Future;
use std::sync::Arc;

use telarel_common::HookUsage;

use crate::_types::context::{CommonPluginContext, PluginContext};
use crate::_types::hooks::end::EndArgs;
use crate::_types::hooks::finalize::FinalizeArgs;
use crate::_types::hooks::generate::{GenerateArgs, GenerateReturn};
use crate::_types::hooks::notify::NotifyReturn;
use crate::_types::hooks::options::{OptionsArgs, OptionsReturn};
use crate::_types::hooks::parse::{ParseArgs, ParseReturn};
use crate::_types::hooks::prepare::PrepareArgs;
use crate::_types::hooks::start::StartArgs;
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

    /// Run the `start` hook with the fully resolved, read-only options;
    /// notify-only.
    fn start<'a>(
        &'a self,
        _ctx: &'a PluginContext<'_>,
        _args: &'a StartArgs,
    ) -> impl Future<Output = NotifyReturn> + Send {
        async { Ok(()) }
    }

    /// Ordering for the `start` hook; `None` = normal bucket.
    fn start_meta(&self) -> Option<PluginHookMeta> {
        None
    }

    /// Run the `parse` hook. Returning `Some` wins the parse stage and the
    /// rest of the chain is skipped; `None` defers to the next plugin.
    fn parse<'a>(
        &'a self,
        _ctx: &'a PluginContext<'_>,
        _args: ParseArgs<'a>,
    ) -> impl Future<Output = ParseReturn> + Send + 'a {
        async { Ok(None) }
    }

    /// Ordering for the `parse` hook; `None` = normal bucket.
    fn parse_meta(&self) -> Option<PluginHookMeta> {
        None
    }

    /// Run the `prepare` hook as a read-only observer of the parsed AST;
    /// notify-only.
    fn prepare<'a>(
        &'a self,
        _ctx: &'a PluginContext<'_>,
        _args: PrepareArgs<'a>,
    ) -> impl Future<Output = NotifyReturn> + Send + 'a {
        async { Ok(()) }
    }

    /// Ordering for the `prepare` hook; `None` = normal bucket.
    fn prepare_meta(&self) -> Option<PluginHookMeta> {
        None
    }

    /// Run the `transform` hook. The hook receives an owned cheap AST handle;
    /// mutating it through `Ast::with_mut` copies on write. Returning
    /// `Some(TransformOutput)` replaces the carried AST; `None` leaves the
    /// driver's AST untouched.
    fn transform<'a>(
        &'a self,
        _ctx: &'a PluginContext<'_>,
        _args: TransformArgs,
    ) -> impl Future<Output = TransformReturn> + Send + 'a {
        async move { Ok(None) }
    }

    /// Ordering for the `transform` hook; `None` = normal bucket.
    fn transform_meta(&self) -> Option<PluginHookMeta> {
        None
    }

    /// Run the `finalize` hook as a read-only observer of the final AST;
    /// notify-only.
    fn finalize<'a>(
        &'a self,
        _ctx: &'a PluginContext<'_>,
        _args: FinalizeArgs<'a>,
    ) -> impl Future<Output = NotifyReturn> + Send + 'a {
        async { Ok(()) }
    }

    /// Ordering for the `finalize` hook; `None` = normal bucket.
    fn finalize_meta(&self) -> Option<PluginHookMeta> {
        None
    }

    /// Run the `generate` hook. Returning `Some` wins the generate stage and
    /// the rest of the chain is skipped; `None` defers to the next plugin.
    fn generate<'a>(
        &'a self,
        _ctx: &'a PluginContext<'_>,
        _args: GenerateArgs<'a>,
    ) -> impl Future<Output = GenerateReturn> + Send + 'a {
        async { Ok(None) }
    }

    /// Ordering for the `generate` hook; `None` = normal bucket.
    fn generate_meta(&self) -> Option<PluginHookMeta> {
        None
    }

    /// Run the `end` hook with the compile error, if any; notify-only. Core
    /// must invoke it on success and on every error path.
    fn end<'a>(
        &'a self,
        _ctx: &'a PluginContext<'_>,
        _args: &'a EndArgs,
    ) -> impl Future<Output = NotifyReturn> + Send {
        async { Ok(()) }
    }

    /// Ordering for the `end` hook; `None` = normal bucket.
    fn end_meta(&self) -> Option<PluginHookMeta> {
        None
    }
}

#[cfg(test)]
#[allow(clippy::manual_async_fn)]
mod tests {
    use std::borrow::Cow;
    use std::future::Future;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::sync::Mutex;

    use oxc::ast::ast::{Directive, Program, StringLiteral};
    use oxc::ast::builder::AstBuilder;
    use oxc::span::SPAN;
    use telarel_common::{
        Ast, AstResult, CodeResult, CompileError, HookUsage, Language,
        ParseAstOptions, ParserOptions, ResolvableOptions, SourceMapOptions,
        SourceType, parse_ast,
    };

    use crate::_types::context::{
        CommonPluginContext, ModuleInfo, PluginContext,
    };
    use crate::_types::hooks::start::StartArgs;
    use crate::_types::hooks::transform::{TransformArgs, TransformOutput};
    use crate::PluginError;
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

                next.options.file = Some(self.file.to_string());

                Ok(Some(next))
            }
        }
    }

    #[derive(Debug)]
    struct PrepareObserverPlugin;

    impl Plugin for PrepareObserverPlugin {
        fn name(&self) -> Cow<'static, str> {
            "prepare-observer".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Prepare
        }

        fn prepare<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            _args: PrepareArgs<'a>,
        ) -> impl Future<Output = NotifyReturn> + Send + 'a {
            async { Ok(()) }
        }
    }

    #[derive(Debug)]
    struct FinalizeObserverPlugin;

    impl Plugin for FinalizeObserverPlugin {
        fn name(&self) -> Cow<'static, str> {
            "finalize-observer".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Finalize
        }

        fn finalize<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            _args: FinalizeArgs<'a>,
        ) -> impl Future<Output = NotifyReturn> + Send + 'a {
            async { Ok(()) }
        }
    }

    fn parse_test_ast(code: &str) -> Ast {
        parse_ast(
            code.to_string(),
            ParseAstOptions {
                cwd: PathBuf::from("/repo"),
                file: String::from("a.ts"),
                language: None,
                source_type: None,
                input_map: None,
            },
        )
        .unwrap()
    }

    #[derive(Debug)]
    struct ParsePlugin;

    impl Plugin for ParsePlugin {
        fn name(&self) -> Cow<'static, str> {
            "parse".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Parse
        }

        fn parse<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            args: ParseArgs<'a>,
        ) -> impl Future<Output = ParseReturn> + Send + 'a {
            async move {
                let ast: Ast = parse_test_ast(args.code);

                Ok(Some(AstResult { ast }))
            }
        }
    }

    #[derive(Debug)]
    struct GeneratePlugin;

    impl Plugin for GeneratePlugin {
        fn name(&self) -> Cow<'static, str> {
            "generate".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Generate
        }

        fn generate<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            _args: GenerateArgs<'a>,
        ) -> impl Future<Output = GenerateReturn> + Send + 'a {
            async {
                Ok(Some(CodeResult {
                    code: String::from("generated;"),
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

        fn transform<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            args: TransformArgs,
        ) -> impl Future<Output = TransformReturn> + Send + 'a {
            async move {
                let mut ast: Ast = args.ast;

                ast.with_mut(|allocator, program: &mut Program<'_>| {
                    let builder: AstBuilder<'_> = AstBuilder::new(allocator);

                    let string_literal: StringLiteral<'_> =
                        StringLiteral::new(SPAN, "\"mark\";", None, &builder);

                    let directive: Directive<'_> =
                        Directive::new(SPAN, string_literal, "mark", &builder);

                    program.directives.push(directive);
                });

                Ok(Some(TransformOutput { ast }))
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

        fn transform<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            _args: TransformArgs,
        ) -> impl Future<Output = TransformReturn> + Send + 'a {
            async move {
                let code: &str = "\"replaced\";";

                let ast: Ast = parse_ast(
                    code.to_string(),
                    ParseAstOptions {
                        cwd: PathBuf::from("/repo"),
                        file: String::from("a.ts"),
                        language: None,
                        source_type: None,
                        input_map: None,
                    },
                )
                .map_err(|error: CompileError| {
                    PluginError::new(error.to_string())
                })?;

                Ok(Some(TransformOutput { ast }))
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
            async { Err(PluginError::new("boom")) }
        }
    }

    type EndedLog = Vec<Option<String>>;

    #[derive(Debug)]
    struct NotifyPlugin {
        started: Arc<Mutex<Option<PathBuf>>>,
        ended: Arc<Mutex<EndedLog>>,
    }

    impl Plugin for NotifyPlugin {
        fn name(&self) -> Cow<'static, str> {
            "notify".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Start | HookUsage::End
        }

        fn start<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            args: &'a StartArgs,
        ) -> impl Future<Output = NotifyReturn> + Send {
            async move {
                *self.started.lock().unwrap() = Some(args.options.cwd.clone());

                Ok(())
            }
        }

        fn end<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            args: &'a EndArgs,
        ) -> impl Future<Output = NotifyReturn> + Send {
            async move {
                self.ended
                    .lock()
                    .unwrap()
                    .push(args.error.as_ref().map(ToString::to_string));

                Ok(())
            }
        }
    }

    #[derive(Debug)]
    struct ContextObserverPlugin {
        observed: Arc<Mutex<Option<(PathBuf, String)>>>,
    }

    impl Plugin for ContextObserverPlugin {
        fn name(&self) -> Cow<'static, str> {
            "context-observer".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Start
        }

        fn start<'a>(
            &'a self,
            ctx: &'a PluginContext<'_>,
            _args: &'a StartArgs,
        ) -> impl Future<Output = NotifyReturn> + Send {
            async move {
                *self.observed.lock().unwrap() =
                    Some((ctx.cwd.to_path_buf(), ctx.module.file.to_string()));

                Ok(())
            }
        }
    }

    fn make_context<'a>() -> PluginContext<'a> {
        let module: ModuleInfo<'a> = ModuleInfo {
            file: "a.ts",
            language: Language::TS,
            source_type: SourceType::Module,
        };

        PluginContext::new(Path::new("/repo"), module)
    }

    fn make_args(plugins: Vec<SharedPluginable>) -> OptionsArgs {
        OptionsArgs {
            options: ResolvableOptions {
                file: Some("a.ts".into()),
                ..ResolvableOptions::default()
            },
            plugins,
        }
    }

    fn parser_options() -> ParserOptions {
        ParserOptions {
            cwd: PathBuf::from("/repo"),
            file: String::from("a.ts"),
            language: Language::TS,
            source_type: SourceType::Module,
            source_map: SourceMapOptions::default(),
        }
    }

    fn make_start_args() -> StartArgs {
        StartArgs {
            options: telarel_common::ResolvedOptions {
                cwd: PathBuf::from("/resolved"),
                file: String::from("a.ts"),
                language: Language::TS,
                source_type: SourceType::Module,
                output: telarel_common::ResolvedOutputOptions {
                    file: String::from("dist/a.js"),
                },
                source_map: SourceMapOptions::default(),
                plugins: vec![String::from("probe")],
            },
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

        let ctx: PluginContext<'_> = make_context();

        let ast: Ast = parse_test_ast("console.log(1);");

        let shared: SharedPluginable = Plugin::new_shared(ProbePlugin);

        let args: OptionsArgs = make_args(vec![]);

        let options: ParserOptions = parser_options();

        let generator: telarel_common::GeneratorOptions =
            telarel_common::GeneratorOptions {
                cwd: PathBuf::from("/repo"),
                file: String::from("a.ts"),
                source_map: SourceMapOptions::default(),
            };

        assert!(shared.call_options(&common, &args).await.unwrap().is_none());
        assert!(
            shared
                .call_parse(
                    &ctx,
                    ParseArgs { code: "let a;", options: &options }
                )
                .await
                .unwrap()
                .is_none()
        );
        shared.call_prepare(&ctx, PrepareArgs { ast: &ast }).await.unwrap();
        assert!(
            shared
                .call_transform(&ctx, TransformArgs { ast: ast.clone() })
                .await
                .unwrap()
                .is_none()
        );
        shared.call_finalize(&ctx, FinalizeArgs { ast: &ast }).await.unwrap();
        assert!(
            shared
                .call_generate(
                    &ctx,
                    GenerateArgs { ast: &ast, options: &generator }
                )
                .await
                .unwrap()
                .is_none()
        );
        shared.call_start(&ctx, &make_start_args()).await.unwrap();
        shared.call_end(&ctx, &EndArgs { error: None }).await.unwrap();
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

        assert_eq!(next.options.file.as_deref(), Some("b.ts"));
        assert!(next.plugins.is_empty());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_options_error_propagates() {
        let common: CommonPluginContext = CommonPluginContext::default();

        let args: OptionsArgs = make_args(vec![]);

        let shared: SharedPluginable = Plugin::new_shared(FailingOptionsPlugin);

        let err: PluginError =
            shared.call_options(&common, &args).await.unwrap_err();

        assert!(err.message().contains("boom"), "{err}");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_parse_returns_ast_result() {
        let ctx: PluginContext<'_> = make_context();

        let options: ParserOptions = parser_options();

        let shared: SharedPluginable = Plugin::new_shared(ParsePlugin);

        let result: Option<AstResult> = shared
            .call_parse(&ctx, ParseArgs { code: "let b;", options: &options })
            .await
            .unwrap();

        let result: AstResult = result.expect("parse plugin returns Some");

        assert_eq!(result.ast.source().as_ref(), "let b;");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_generate_returns_code_result() {
        let ctx: PluginContext<'_> = make_context();

        let ast: Ast = parse_test_ast("console.log(1);");

        let generator: telarel_common::GeneratorOptions =
            telarel_common::GeneratorOptions {
                cwd: PathBuf::from("/repo"),
                file: String::from("a.ts"),
                source_map: SourceMapOptions::default(),
            };

        let shared: SharedPluginable = Plugin::new_shared(GeneratePlugin);

        let result: Option<CodeResult> = shared
            .call_generate(
                &ctx,
                GenerateArgs { ast: &ast, options: &generator },
            )
            .await
            .unwrap();

        assert_eq!(
            result.expect("generate plugin returns Some").code,
            "generated;"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_prepare_and_finalize_are_observers() {
        let ctx: PluginContext<'_> = make_context();

        let ast: Ast = parse_test_ast("console.log(1);");

        let prepare: SharedPluginable =
            Plugin::new_shared(PrepareObserverPlugin);

        prepare.call_prepare(&ctx, PrepareArgs { ast: &ast }).await.unwrap();

        let finalize: SharedPluginable =
            Plugin::new_shared(FinalizeObserverPlugin);

        finalize.call_finalize(&ctx, FinalizeArgs { ast: &ast }).await.unwrap();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_transform_none_keeps_original_program() {
        let ctx: PluginContext<'_> = make_context();

        let ast: Ast = parse_test_ast("console.log(1);");

        let args: TransformArgs = TransformArgs { ast: ast.clone() };

        let shared: SharedPluginable = Plugin::new_shared(ProbePlugin);

        let result: Option<TransformOutput> =
            shared.call_transform(&ctx, args).await.unwrap();

        assert!(result.is_none());

        let out: String =
            oxc::codegen::Codegen::new().build(ast.program()).code;

        assert!(out.contains("console.log"), "{out}");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_transform_clone_mutation_is_returned() {
        let ctx: PluginContext<'_> = make_context();

        let ast: Ast = parse_test_ast("console.log(1);");

        let args: TransformArgs = TransformArgs { ast: ast.clone() };

        let shared: SharedPluginable = Plugin::new_shared(CloneMutatePlugin);

        let result: Option<TransformOutput> =
            shared.call_transform(&ctx, args).await.unwrap();

        let output: TransformOutput =
            result.expect("clone-mutate must return Some");

        let out: String =
            oxc::codegen::Codegen::new().build(output.ast.program()).code;

        assert!(out.contains("mark"), "{out}");

        let original: String =
            oxc::codegen::Codegen::new().build(ast.program()).code;

        assert!(
            !original.contains("mark"),
            "the input program must stay untouched: {original}"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_transform_some_returns_replaced_program() {
        let ctx: PluginContext<'_> = make_context();

        let ast: Ast = parse_test_ast("console.log(1);");

        let args: TransformArgs = TransformArgs { ast: ast.clone() };

        let shared: SharedPluginable = Plugin::new_shared(ReplaceRootPlugin);

        let output: TransformOutput =
            shared.call_transform(&ctx, args).await.unwrap().unwrap();

        let out: String =
            oxc::codegen::Codegen::new().build(output.ast.program()).code;

        assert!(out.contains("replaced"), "{out}");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_start_and_end_receive_args() {
        let started: Arc<Mutex<Option<PathBuf>>> = Arc::new(Mutex::new(None));

        let ended: Arc<Mutex<EndedLog>> = Arc::new(Mutex::new(Vec::new()));

        let ctx: PluginContext<'_> = make_context();

        let shared: SharedPluginable = Plugin::new_shared(NotifyPlugin {
            started: Arc::clone(&started),
            ended: Arc::clone(&ended),
        });

        shared.call_start(&ctx, &make_start_args()).await.unwrap();

        shared
            .call_end(
                &ctx,
                &EndArgs {
                    error: Some(telarel_common::CompileError::new("boom")),
                },
            )
            .await
            .unwrap();

        assert_eq!(
            started.lock().unwrap().as_deref(),
            Some(Path::new("/resolved"))
        );

        assert_eq!(*ended.lock().unwrap(), vec![Some(String::from("boom"))]);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_plugin_observes_context_cwd_and_module_file() {
        let observed: Arc<Mutex<Option<(PathBuf, String)>>> =
            Arc::new(Mutex::new(None));

        let ctx: PluginContext<'_> = make_context();

        let shared: SharedPluginable =
            Plugin::new_shared(ContextObserverPlugin {
                observed: Arc::clone(&observed),
            });

        shared.call_start(&ctx, &make_start_args()).await.unwrap();

        assert_eq!(
            *observed.lock().unwrap(),
            Some((PathBuf::from("/repo"), String::from("a.ts")))
        );
    }
}
