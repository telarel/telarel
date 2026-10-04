pub mod hooks;

use std::sync::Arc;

use telarel_common::HookUsage;

use crate::_types::context::PluginContext;
use crate::_types::hooks::compile_end::CompileEndArgs;
use crate::_types::hooks::compile_start::CompileStartArgs;
use crate::_types::hooks::finalize::FinalizeArgs;
use crate::_types::hooks::prepare::PrepareArgs;
use crate::_types::hooks::transform::{TransformArgs, TransformOutput};
use crate::_types::order::{PluginHookMeta, PluginOrder};
use crate::plugin::pluginable::SharedPluginable;

/// Order plugins by their per-hook meta: `[pre, normal, post]`, stable within a
/// bucket. A plugin whose hook meta is `None`, or whose meta carries
/// `order: None`, lands in the normal bucket.
pub fn sort_plugins_by_hook_meta(
    plugins: &[SharedPluginable],
    get_hook_meta: fn(&SharedPluginable) -> Option<PluginHookMeta>,
) -> Vec<SharedPluginable> {
    let mut pre: Vec<SharedPluginable> = Vec::new();
    let mut normal: Vec<SharedPluginable> = Vec::new();
    let mut post: Vec<SharedPluginable> = Vec::new();

    for plugin in plugins {
        match get_hook_meta(plugin).and_then(|meta| meta.order) {
            | Some(PluginOrder::Pre) => pre.push(Arc::clone(plugin)),
            | Some(PluginOrder::Post) => post.push(Arc::clone(plugin)),
            | None => normal.push(Arc::clone(plugin)),
        }
    }

    [pre, normal, post].concat()
}

/// Drives plugins through the hooks, each ordered by its per-hook meta
/// (`[pre, normal, post]`, stable within a bucket).
pub struct PluginDriver {
    usage: HookUsage,
    settled_names: Vec<String>,
    compile_start_plugins: Vec<SharedPluginable>,
    prepare_plugins: Vec<SharedPluginable>,
    transform_plugins: Vec<SharedPluginable>,
    finalize_plugins: Vec<SharedPluginable>,
    compile_end_plugins: Vec<SharedPluginable>,
}

impl PluginDriver {
    /// Create a driver from plugins in registration order.
    ///
    /// Plugins are partitioned by their declared [`HookUsage`];
    /// a hook only iterates the plugins that declared it. There is no
    /// `options` partition: the `options` stage is the fixpoint runner
    /// ([`crate::options_fixpoint`]), not the driver.
    pub fn new(plugins: Vec<SharedPluginable>) -> Self {
        let mut usage: HookUsage = HookUsage::default();

        let mut prepare_plugins: Vec<SharedPluginable> = Vec::new();
        let mut transform_plugins: Vec<SharedPluginable> = Vec::new();
        let mut finalize_plugins: Vec<SharedPluginable> = Vec::new();

        for plugin in &plugins {
            let declared: HookUsage = plugin.call_register_hook_usage();

            usage |= declared;

            if declared.contains(HookUsage::Prepare) {
                prepare_plugins.push(Arc::clone(plugin));
            }

            if declared.contains(HookUsage::Transform) {
                transform_plugins.push(Arc::clone(plugin));
            }

            if declared.contains(HookUsage::Finalize) {
                finalize_plugins.push(Arc::clone(plugin));
            }
        }

        let compile_start_plugins: Vec<SharedPluginable> =
            sort_plugins_by_hook_meta(&plugins, |plugin| {
                plugin.call_compile_start_meta()
            });

        let prepare_plugins: Vec<SharedPluginable> =
            sort_plugins_by_hook_meta(&prepare_plugins, |plugin| {
                plugin.call_prepare_meta()
            });

        let transform_plugins: Vec<SharedPluginable> =
            sort_plugins_by_hook_meta(&transform_plugins, |plugin| {
                plugin.call_transform_meta()
            });

        let finalize_plugins: Vec<SharedPluginable> =
            sort_plugins_by_hook_meta(&finalize_plugins, |plugin| {
                plugin.call_finalize_meta()
            });

        let compile_end_plugins: Vec<SharedPluginable> =
            sort_plugins_by_hook_meta(&plugins, |plugin| {
                plugin.call_compile_end_meta()
            });

        let settled_names: Vec<String> = plugins
            .iter()
            .map(|plugin| plugin.call_name().into_owned())
            .collect();

        Self {
            usage,
            settled_names,
            compile_start_plugins,
            prepare_plugins,
            transform_plugins,
            finalize_plugins,
            compile_end_plugins,
        }
    }

    /// The aggregate hook usage across all plugins.
    pub fn usage(&self) -> HookUsage {
        self.usage
    }

    /// The settled plugin names in registration order; duplicates preserved.
    pub fn settled_names(&self) -> &[String] {
        &self.settled_names
    }

    /// Run the `compile_start` hook on EVERY settled plugin (not partitioned
    /// by usage); the first error aborts. Notify-only.
    pub async fn compile_start(
        &self,
        ctx: &PluginContext<'_>,
        args: &CompileStartArgs,
    ) -> anyhow::Result<()> {
        hooks::compile_start::compile_start(
            &self.compile_start_plugins,
            ctx,
            args,
        )
        .await
    }

    /// Run the `prepare` hook on every plugin in meta-ranked order;
    /// `Some` replaces the carried code, `None` keeps it. The fold result
    /// carries every returned map in fold order so the caller composes the
    /// full chain (see [`hooks::prepare::PrepareFold`]).
    pub async fn prepare(
        &self,
        ctx: &PluginContext<'_>,
        args: &PrepareArgs<'_>,
    ) -> anyhow::Result<hooks::prepare::PrepareFold> {
        hooks::prepare::prepare(&self.prepare_plugins, ctx, args).await
    }

    /// Run the `transform` hook chain;
    /// `Some` carries the last returned program, `None` = unchanged.
    pub async fn transform<'a, 'ast: 'a>(
        &'a self,
        ctx: &'a PluginContext<'a>,
        args: &TransformArgs<'ast>,
    ) -> anyhow::Result<Option<TransformOutput<'ast>>> {
        hooks::transform::transform(&self.transform_plugins, ctx, args).await
    }

    /// Run the `finalize` hook on every plugin in meta-ranked order;
    /// `Some` replaces the carried code, `None` keeps it. The fold result
    /// carries every returned map in fold order so the caller composes the
    /// full chain (see [`hooks::finalize::FinalizeFold`]).
    pub async fn finalize(
        &self,
        ctx: &PluginContext<'_>,
        args: &FinalizeArgs<'_>,
    ) -> anyhow::Result<hooks::finalize::FinalizeFold> {
        hooks::finalize::finalize(&self.finalize_plugins, ctx, args).await
    }

    /// Run the `compile_end` hook on EVERY settled plugin (not partitioned by
    /// usage); every hook runs even after one fails, and the first error is
    /// returned after the loop. Notify-only.
    pub async fn compile_end(
        &self,
        ctx: &PluginContext<'_>,
        args: &CompileEndArgs,
    ) -> anyhow::Result<()> {
        hooks::compile_end::compile_end(&self.compile_end_plugins, ctx, args)
            .await
    }
}

// The house style for the hook impls is the explicit RPITIT form (never
// `async fn`), so the `manual_async_fn` lint is suppressed here.
#[cfg(test)]
#[allow(clippy::manual_async_fn)]
mod tests {
    use std::borrow::Cow;
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::sync::MutexGuard;

    use oxc::allocator::Allocator;
    use oxc::allocator::CloneIn;
    use oxc::ast::ast::{Directive, Program, StringLiteral};
    use oxc::ast::builder::AstBuilder;
    use oxc::span::SPAN;

    use telarel_common::{
        CompileContext, HookUsage, Language, ParseOptions, ParseResult,
        SourceType, parse,
    };

    use crate::_types::context::{CommonPluginContext, ModuleInfo};
    use crate::_types::hooks::finalize::FinalizeReturn;
    use crate::_types::hooks::notify::NotifyReturn;
    use crate::_types::hooks::prepare::PrepareReturn;
    use crate::_types::hooks::transform::{TransformArgs, TransformReturn};
    use crate::_types::order::{PluginHookMeta, PluginOrder};
    use crate::SharedPluginable;
    use crate::plugin::Plugin;
    use crate::plugin::pluginable::Pluginable;

    use super::*;

    fn common_ctx() -> CommonPluginContext {
        CommonPluginContext::default()
    }

    fn make_module<'a>(code: &'a str) -> ModuleInfo<'a> {
        ModuleInfo {
            file: "a.ts",
            code,
            language: Language::TS,
            source_type: SourceType::Module,
        }
    }

    fn test_parse_program<'a>(allocator: &'a Allocator) -> ParseResult<'a> {
        let ctx: telarel_common::CompileContext<'_> =
            telarel_common::CompileContext::new(
                "/repo",
                "a.ts",
                "console.log(1);",
            );

        parse(ParseOptions {
            context: &ctx,
            allocator,
            file: "a.ts",
            code: "console.log(1);",
            language: None,
            source_type: None,
        })
        .unwrap()
    }

    #[derive(Debug)]
    struct OrderPlugin {
        name: &'static str,
        log: Arc<Mutex<Vec<String>>>,
    }

    impl OrderPlugin {
        fn new(
            name: &'static str,
            log: Arc<Mutex<Vec<String>>>,
        ) -> Self {
            Self { name, log }
        }

        fn record(&self) {
            let mut log: MutexGuard<'_, Vec<String>> = self.log.lock().unwrap();
            log.push(self.name.to_string());
        }
    }

    impl Plugin for OrderPlugin {
        fn name(&self) -> Cow<'static, str> {
            self.name.into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Prepare | HookUsage::Finalize
        }

        fn prepare<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            _args: &'a PrepareArgs<'_>,
        ) -> impl Future<Output = PrepareReturn> + Send {
            async move {
                self.record();

                Ok(None)
            }
        }

        fn finalize<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            _args: &'a FinalizeArgs<'_>,
        ) -> impl Future<Output = FinalizeReturn> + Send {
            async move {
                self.record();

                Ok(None)
            }
        }
    }

    #[derive(Debug)]
    struct FailingPreparePlugin;

    impl Plugin for FailingPreparePlugin {
        fn name(&self) -> Cow<'static, str> {
            "fail-prepare".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Prepare
        }

        fn prepare<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            _args: &'a PrepareArgs<'_>,
        ) -> impl Future<Output = PrepareReturn> + Send {
            async move { Err(anyhow::anyhow!("boom")) }
        }
    }

    #[derive(Debug)]
    struct MarkPlugin;

    impl Plugin for MarkPlugin {
        fn name(&self) -> Cow<'static, str> {
            "mark".into()
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
                let code: &'ast str = args.allocator.alloc_str("\"mark\";");

                let file: &'ast str =
                    args.allocator.alloc_str(_ctx.module.file);

                let ctx: CompileContext<'_> =
                    CompileContext::new("/repo", file, code);

                let parsed: ParseResult<'ast> = parse(ParseOptions {
                    context: &ctx,
                    allocator: args.allocator,
                    file,
                    code,
                    language: None,
                    source_type: None,
                })?;

                let rooted: &'ast Program<'ast> =
                    args.allocator.alloc(parsed.program);

                Ok(Some(crate::TransformOutput { ast: rooted }))
            }
        }
    }

    #[derive(Debug)]
    struct RecordingPlugin {
        seen: Arc<Mutex<Vec<usize>>>,
    }

    impl RecordingPlugin {
        fn new(seen: Arc<Mutex<Vec<usize>>>) -> Self {
            Self { seen }
        }

        fn record(
            &self,
            program: &Program<'_>,
        ) {
            let mut seen: MutexGuard<'_, Vec<usize>> =
                self.seen.lock().unwrap();
            seen.push(std::ptr::from_ref(program) as usize);
        }
    }

    impl Plugin for RecordingPlugin {
        fn name(&self) -> Cow<'static, str> {
            "record-transform".into()
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
                self.record(args.ast);

                Ok(None)
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
                    CompileContext::new("/repo", file, code);

                let parsed: ParseResult<'_> = parse(ParseOptions {
                    context: &ctx,
                    allocator: args.allocator,
                    file,
                    code,
                    language: None,
                    source_type: None,
                })?;

                let fresh: &'ast Program<'ast> =
                    args.allocator.alloc(parsed.program);

                Ok(Some(crate::TransformOutput { ast: fresh }))
            }
        }
    }

    #[derive(Debug)]
    struct AppendDirectivePlugin;

    const APPEND_DIRECTIVE: &str = "x-appended";

    impl Plugin for AppendDirectivePlugin {
        fn name(&self) -> Cow<'static, str> {
            "append-directive".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Transform
        }

        // Clone the carried program, append a directive, and return the
        // clone.
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
                    StringLiteral::new(SPAN, APPEND_DIRECTIVE, None, &builder);

                let directive: Directive<'ast> = Directive::new(
                    SPAN,
                    string_literal,
                    APPEND_DIRECTIVE,
                    &builder,
                );

                working.directives.push(directive);

                let rooted: &'ast Program<'ast> = args.allocator.alloc(working);

                Ok(Some(crate::TransformOutput { ast: rooted }))
            }
        }
    }

    #[derive(Debug)]
    struct FailingTransformPlugin;

    impl Plugin for FailingTransformPlugin {
        fn name(&self) -> Cow<'static, str> {
            "fail-transform".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Transform
        }

        fn transform<'a, 'ast: 'a>(
            &'a self,
            _ctx: &'a PluginContext<'a>,
            _args: TransformArgs<'ast>,
        ) -> impl Future<Output = TransformReturn<'ast>> + 'a {
            async move { Err(anyhow::anyhow!("boom")) }
        }
    }

    #[derive(Debug)]
    struct FailingFinalizePlugin;

    impl Plugin for FailingFinalizePlugin {
        fn name(&self) -> Cow<'static, str> {
            "fail-finalize".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Finalize
        }

        fn finalize(
            &self,
            _ctx: &PluginContext<'_>,
            _args: &FinalizeArgs<'_>,
        ) -> impl Future<Output = FinalizeReturn> + Send {
            async move { Err(anyhow::anyhow!("boom")) }
        }
    }

    #[derive(Debug)]
    struct PrepareOnlyPlugin {
        name: &'static str,
        log: Arc<Mutex<Vec<String>>>,
    }

    impl PrepareOnlyPlugin {
        fn new(log: Arc<Mutex<Vec<String>>>) -> Self {
            Self { name: "prepare-only", log }
        }

        fn record(&self) {
            let mut log: MutexGuard<'_, Vec<String>> = self.log.lock().unwrap();
            log.push(self.name.to_string());
        }
    }

    impl Plugin for PrepareOnlyPlugin {
        fn name(&self) -> Cow<'static, str> {
            self.name.into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Prepare
        }

        fn prepare<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            _args: &'a PrepareArgs<'_>,
        ) -> impl Future<Output = PrepareReturn> + Send {
            async move {
                self.record();

                Ok(None)
            }
        }
    }

    fn make_plugin_ctx<'a>(
        common: &'a CommonPluginContext
    ) -> PluginContext<'a> {
        PluginContext::new(
            &common.state,
            "/repo",
            make_module("console.log(1);"),
        )
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_prepare_runs_all_plugins_in_order() {
        let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(OrderPlugin::new("first", Arc::clone(&log))),
            Plugin::new_shared(OrderPlugin::new("second", Arc::clone(&log))),
        ];

        let driver: PluginDriver = PluginDriver::new(plugins);

        let common: CommonPluginContext = common_ctx();

        let ctx: PluginContext<'_> = make_plugin_ctx(&common);

        let args: PrepareArgs<'_> = PrepareArgs { code: "console.log(1);" };

        let fold: hooks::prepare::PrepareFold =
            driver.prepare(&ctx, &args).await.unwrap();

        assert!(fold.output.is_none());

        assert!(fold.maps.is_empty());

        let recorded: Vec<String> = log.lock().unwrap().clone();

        assert_eq!(recorded, vec!["first".to_string(), "second".to_string()]);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_prepare_error_aborts_and_names_plugin() {
        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(OrderPlugin::new(
                "first",
                Arc::new(Mutex::new(Vec::new())),
            )),
            Plugin::new_shared(FailingPreparePlugin),
        ];

        let driver: PluginDriver = PluginDriver::new(plugins);

        let common: CommonPluginContext = common_ctx();

        let ctx: PluginContext<'_> = make_plugin_ctx(&common);

        let args: PrepareArgs<'_> = PrepareArgs { code: "console.log(1);" };

        let err: anyhow::Error = driver.prepare(&ctx, &args).await.unwrap_err();

        assert!(
            format!("{err:#}").contains("`fail-prepare` prepare"),
            "{err:#}"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_transform_chains_programs() {
        let allocator: Allocator = Allocator::default();

        let common: CommonPluginContext = common_ctx();

        let ctx: PluginContext<'_> = make_plugin_ctx(&common);

        let parsed: ParseResult<'_> = test_parse_program(&allocator);

        let program: Program<'_> = parsed.program;

        let args: TransformArgs<'_> =
            TransformArgs { allocator: &allocator, ast: &program };

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(MarkPlugin),
            Plugin::new_shared(MarkPlugin),
        ];

        let driver: PluginDriver = PluginDriver::new(plugins);

        let output: Option<crate::TransformOutput<'_>> =
            driver.transform(&ctx, &args).await.unwrap();

        let output: crate::TransformOutput<'_> =
            output.expect("the chain changed the program");

        let out: String = oxc::codegen::Codegen::new().build(output.ast).code;

        assert!(out.contains("mark"), "{out}");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_transform_error_names_plugin() {
        let allocator: Allocator = Allocator::default();

        let common: CommonPluginContext = common_ctx();

        let ctx: PluginContext<'_> = make_plugin_ctx(&common);

        let parsed: ParseResult<'_> = test_parse_program(&allocator);

        let program: Program<'_> = parsed.program;

        let args: TransformArgs<'_> =
            TransformArgs { allocator: &allocator, ast: &program };

        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(FailingTransformPlugin)];

        let driver: PluginDriver = PluginDriver::new(plugins);

        let err: anyhow::Error =
            driver.transform(&ctx, &args).await.unwrap_err();

        assert!(
            format!("{err:#}").contains("`fail-transform` transform"),
            "{err:#}"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_transform_returns_original_program_by_reference() {
        let allocator: Allocator = Allocator::default();

        let common: CommonPluginContext = common_ctx();

        let ctx: PluginContext<'_> = make_plugin_ctx(&common);

        let parsed: ParseResult<'_> = test_parse_program(&allocator);

        let program: Program<'_> = parsed.program;

        let args: TransformArgs<'_> =
            TransformArgs { allocator: &allocator, ast: &program };

        let seen: Arc<Mutex<Vec<usize>>> = Arc::new(Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(RecordingPlugin::new(Arc::clone(&seen)))];

        let driver: PluginDriver = PluginDriver::new(plugins);

        let output: Option<crate::TransformOutput<'_>> =
            driver.transform(&ctx, &args).await.unwrap();

        assert!(output.is_none());

        let recorded: Vec<usize> = seen.lock().unwrap().clone();

        // no plugin returned `Some`, so the fold must hand the original
        // program to the plugin by reference instead of cloning it
        assert_eq!(recorded, vec![std::ptr::from_ref(&program) as usize],);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_transform_some_program_reaches_later_plugins() {
        // A `Some`-returning plugin's program is what the following plugin
        // receives AND what the fold returns. The regression this pins: a
        // fold that kept handing later plugins the original program.
        let allocator: Allocator = Allocator::default();

        let common: CommonPluginContext = common_ctx();

        let ctx: PluginContext<'_> = make_plugin_ctx(&common);

        let parsed: ParseResult<'_> = test_parse_program(&allocator);

        let program: Program<'_> = parsed.program;

        let args: TransformArgs<'_> =
            TransformArgs { allocator: &allocator, ast: &program };

        let seen: Arc<Mutex<Vec<usize>>> = Arc::new(Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(ReplaceRootPlugin),
            Plugin::new_shared(RecordingPlugin::new(Arc::clone(&seen))),
        ];

        let driver: PluginDriver = PluginDriver::new(plugins);

        let output: Option<crate::TransformOutput<'_>> =
            driver.transform(&ctx, &args).await.unwrap();

        let output: crate::TransformOutput<'_> =
            output.expect("the chain changed the program");

        // the following plugin observed the replaced program, not the parse
        // result
        let recorded: usize = seen.lock().unwrap()[0];

        assert_eq!(
            recorded,
            std::ptr::from_ref(output.ast) as usize,
            "later plugin must receive the Some-returned program"
        );

        let out: String = oxc::codegen::Codegen::new().build(output.ast).code;

        assert!(out.contains("replaced"), "{out}");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_transform_all_none_returns_none() {
        // An all-`None` chain changes nothing: the fold returns `None` and
        // the carried program stays the original.
        let allocator: Allocator = Allocator::default();

        let common: CommonPluginContext = common_ctx();

        let ctx: PluginContext<'_> = make_plugin_ctx(&common);

        let parsed: ParseResult<'_> = test_parse_program(&allocator);

        let program: Program<'_> = parsed.program;

        let args: TransformArgs<'_> =
            TransformArgs { allocator: &allocator, ast: &program };

        let first_seen: Arc<Mutex<Vec<usize>>> =
            Arc::new(Mutex::new(Vec::new()));

        let second_seen: Arc<Mutex<Vec<usize>>> =
            Arc::new(Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(RecordingPlugin::new(Arc::clone(&first_seen))),
            Plugin::new_shared(RecordingPlugin::new(Arc::clone(&second_seen))),
        ];

        let driver: PluginDriver = PluginDriver::new(plugins);

        let output: Option<crate::TransformOutput<'_>> =
            driver.transform(&ctx, &args).await.unwrap();

        assert!(output.is_none());

        let original: usize = std::ptr::from_ref(&program) as usize;

        assert_eq!(first_seen.lock().unwrap()[0], original);

        assert_eq!(second_seen.lock().unwrap()[0], original);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_transform_last_some_wins() {
        // Two `Some`-returning plugins: the fold returns the LAST one's
        // program, and the earlier replacement is not carried forward into
        // the output.
        let allocator: Allocator = Allocator::default();

        let common: CommonPluginContext = common_ctx();

        let ctx: PluginContext<'_> = make_plugin_ctx(&common);

        let parsed: ParseResult<'_> = test_parse_program(&allocator);

        let program: Program<'_> = parsed.program;

        let args: TransformArgs<'_> =
            TransformArgs { allocator: &allocator, ast: &program };

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(ReplaceRootPlugin),
            Plugin::new_shared(MarkPlugin),
        ];

        let driver: PluginDriver = PluginDriver::new(plugins);

        let output: Option<crate::TransformOutput<'_>> =
            driver.transform(&ctx, &args).await.unwrap();

        let output: crate::TransformOutput<'_> =
            output.expect("the chain changed the program");

        let out: String = oxc::codegen::Codegen::new().build(output.ast).code;

        // the last Some (MarkPlugin's parse of `"mark";`) wins, replacing
        // the earlier `"replaced";` root
        assert!(out.contains("mark"), "{out}");

        assert!(!out.contains("replaced"), "{out}");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_transform_some_then_second_some_keeps_both_edits() {
        // ReplaceRootPlugin then AppendDirectivePlugin: the second clone
        // starts from the replaced root, so the final output carries both
        // the replacement and the appended directive.
        let allocator: Allocator = Allocator::default();

        let common: CommonPluginContext = common_ctx();

        let ctx: PluginContext<'_> = make_plugin_ctx(&common);

        let parsed: ParseResult<'_> = test_parse_program(&allocator);

        let program: Program<'_> = parsed.program;

        let args: TransformArgs<'_> =
            TransformArgs { allocator: &allocator, ast: &program };

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(ReplaceRootPlugin),
            Plugin::new_shared(AppendDirectivePlugin),
        ];

        let driver: PluginDriver = PluginDriver::new(plugins);

        let output: Option<crate::TransformOutput<'_>> =
            driver.transform(&ctx, &args).await.unwrap();

        let output: crate::TransformOutput<'_> =
            output.expect("the chain changed the program");

        let out: String = oxc::codegen::Codegen::new().build(output.ast).code;

        assert!(out.contains("replaced"), "{out}");

        assert!(out.contains(APPEND_DIRECTIVE), "{out}");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_finalize_runs_all_plugins_in_order() {
        let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(OrderPlugin::new("first", Arc::clone(&log))),
            Plugin::new_shared(OrderPlugin::new("second", Arc::clone(&log))),
        ];

        let driver: PluginDriver = PluginDriver::new(plugins);

        let common: CommonPluginContext = common_ctx();

        let ctx: PluginContext<'_> = make_plugin_ctx(&common);

        let args: FinalizeArgs<'_> = FinalizeArgs { code: "console.log(1);" };

        let fold: hooks::finalize::FinalizeFold =
            driver.finalize(&ctx, &args).await.unwrap();

        assert!(fold.output.is_none());

        assert!(fold.maps.is_empty());

        let recorded: Vec<String> = log.lock().unwrap().clone();

        assert_eq!(recorded, vec!["first".to_string(), "second".to_string()]);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_finalize_error_names_plugin() {
        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(OrderPlugin::new(
                "first",
                Arc::new(Mutex::new(Vec::new())),
            )),
            Plugin::new_shared(FailingFinalizePlugin),
        ];

        let driver: PluginDriver = PluginDriver::new(plugins);

        let common: CommonPluginContext = common_ctx();

        let ctx: PluginContext<'_> = make_plugin_ctx(&common);

        let args: FinalizeArgs<'_> = FinalizeArgs { code: "console.log(1);" };

        let err: anyhow::Error =
            driver.finalize(&ctx, &args).await.unwrap_err();

        assert!(
            format!("{err:#}").contains("`fail-finalize` finalize"),
            "{err:#}"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_pluginable_dispatch() {
        let plugin: Arc<dyn Pluginable> = Plugin::new_shared(MarkPlugin);

        assert_eq!(plugin.call_name(), "mark");

        assert!(
            plugin.call_register_hook_usage().contains(HookUsage::Transform)
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_transform_skips_plugins_without_usage() {
        let allocator: Allocator = Allocator::default();

        let common: CommonPluginContext = common_ctx();

        let ctx: PluginContext<'_> = make_plugin_ctx(&common);

        let parsed: ParseResult<'_> = test_parse_program(&allocator);

        let program: Program<'_> = parsed.program;

        let args: TransformArgs<'_> =
            TransformArgs { allocator: &allocator, ast: &program };

        let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(PrepareOnlyPlugin::new(Arc::clone(&log)))];

        let driver: PluginDriver = PluginDriver::new(plugins);

        driver.transform(&ctx, &args).await.unwrap();

        let recorded: Vec<String> = log.lock().unwrap().clone();

        assert_eq!(driver.usage(), HookUsage::Prepare);
        assert!(recorded.is_empty());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_driver_aggregates_usage() {
        // An options-only plugin still contributes its declared bit to the
        // aggregate (no options partition exists — the fixpoint runner owns
        // the options stage).
        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(PrepareOnlyPlugin::new(Arc::new(Mutex::new(
                Vec::new(),
            )))),
            Plugin::new_shared(FailingPreparePlugin),
            Plugin::new_shared(MarkPlugin),
            Plugin::new_shared(FailingFinalizePlugin),
        ];

        let driver: PluginDriver = PluginDriver::new(plugins);

        assert_eq!(
            driver.usage(),
            HookUsage::Prepare | HookUsage::Transform | HookUsage::Finalize
        );
    }

    #[derive(Debug)]
    struct NotifyOrderPlugin {
        name: &'static str,
        stage: &'static str,
        log: Arc<Mutex<Vec<String>>>,
    }

    impl NotifyOrderPlugin {
        fn record(&self) {
            let mut log: MutexGuard<'_, Vec<String>> = self.log.lock().unwrap();
            log.push([self.name.to_string(), self.stage.to_string()].join(":"));
        }
    }

    impl Plugin for NotifyOrderPlugin {
        fn name(&self) -> Cow<'static, str> {
            self.name.into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Prepare
        }

        fn compile_start<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            _args: &'a crate::CompileStartArgs,
        ) -> impl Future<Output = NotifyReturn> {
            async {
                self.record();

                Ok(())
            }
        }

        fn compile_end<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            _args: &'a crate::CompileEndArgs,
        ) -> impl Future<Output = NotifyReturn> {
            async {
                self.record();

                Ok(())
            }
        }
    }

    // Observes the notify-hook args: the compile_start options and the
    // compile_end payload shape.
    #[derive(Debug)]
    struct NotifyArgsPlugin {
        start: Arc<Mutex<Option<String>>>,
        end: Arc<Mutex<Option<String>>>,
    }

    impl Plugin for NotifyArgsPlugin {
        fn name(&self) -> Cow<'static, str> {
            "notify-args".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::CompileStart | HookUsage::CompileEnd
        }

        fn compile_start<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            args: &'a crate::CompileStartArgs,
        ) -> impl Future<Output = NotifyReturn> {
            let cwd: String = args.options.cwd.clone();

            async move {
                *self.start.lock().unwrap() = Some(cwd);

                Ok(())
            }
        }

        fn compile_end<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            args: &'a crate::CompileEndArgs,
        ) -> impl Future<Output = NotifyReturn> {
            let shape: String = format!(
                "{}|{}|{}",
                args.code,
                args.err.as_deref().unwrap_or(""),
                args.map.is_some(),
            );

            async move {
                *self.end.lock().unwrap() = Some(shape);

                Ok(())
            }
        }
    }

    fn make_start_args() -> crate::CompileStartArgs {
        crate::CompileStartArgs {
            options: telarel_common::ResolvedOptions {
                cwd: String::from("/repo"),
                file: String::from("a.ts"),
                code: String::from("console.log(1);"),
                language: Language::TS,
                source_type: SourceType::Module,
                plugins: vec![],
            },
        }
    }

    fn make_end_args() -> crate::CompileEndArgs {
        crate::CompileEndArgs {
            code: String::from("out"),
            map: None,
            err: None,
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_end_receives_payload_shape() {
        // The compile_end args carry the code, the err (unset here), and the
        // map presence flag.
        let end: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

        let start: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(NotifyArgsPlugin {
                start: Arc::clone(&start),
                end: Arc::clone(&end),
            })];

        let driver: PluginDriver = PluginDriver::new(plugins);

        let common: CommonPluginContext = common_ctx();

        let ctx: PluginContext<'_> = make_plugin_ctx(&common);

        driver.compile_start(&ctx, &make_start_args()).await.unwrap();

        assert_eq!(start.lock().unwrap().as_deref(), Some("/repo"));

        driver.compile_end(&ctx, &make_end_args()).await.unwrap();

        let recorded: Option<String> = end.lock().unwrap().clone();

        assert_eq!(recorded.as_deref(), Some("out||false"));
    }

    #[derive(Debug)]
    struct FailingNotifyPlugin {
        hook: &'static str,
    }

    impl Plugin for FailingNotifyPlugin {
        fn name(&self) -> Cow<'static, str> {
            match self.hook {
                | "compile_start" => "fail-start".into(),
                | _ => "fail-end".into(),
            }
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::CompileStart | HookUsage::CompileEnd
        }

        fn compile_start<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            _args: &'a crate::CompileStartArgs,
        ) -> impl Future<Output = NotifyReturn> {
            async {
                if self.hook == "compile_start" {
                    Err(anyhow::anyhow!("boom"))
                } else {
                    Ok(())
                }
            }
        }

        fn compile_end<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            _args: &'a crate::CompileEndArgs,
        ) -> impl Future<Output = NotifyReturn> {
            async {
                if self.hook == "compile_end" {
                    Err(anyhow::anyhow!("boom"))
                } else {
                    Ok(())
                }
            }
        }
    }

    /// A compile_end-only failure carrying an explicit name, so two
    /// failing plugins can be distinguished by name.
    #[derive(Debug)]
    struct FailingNotifyPluginAt {
        name: &'static str,
        ran: Arc<Mutex<bool>>,
    }

    impl Plugin for FailingNotifyPluginAt {
        fn name(&self) -> Cow<'static, str> {
            self.name.into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::CompileEnd
        }

        fn compile_end<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            _args: &'a crate::CompileEndArgs,
        ) -> impl Future<Output = NotifyReturn> {
            let ran: Arc<Mutex<bool>> = Arc::clone(&self.ran);

            async move {
                *ran.lock().unwrap() = true;

                Err(anyhow::anyhow!("boom"))
            }
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_start_runs_all_plugins_not_partitioned() {
        // compile_start reaches plugins that did NOT declare it via usage
        // partitioning... actually it reaches ALL settled plugins: the
        // OrderPlugin declares Prepare|Finalize only, yet its compile_start must
        // still run (R5: invoke on all plugins).
        let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(NotifyOrderPlugin {
                name: "first",
                stage: "start",
                log: Arc::clone(&log),
            }),
            Plugin::new_shared(NotifyOrderPlugin {
                name: "second",
                stage: "start",
                log: Arc::clone(&log),
            }),
        ];

        let driver: PluginDriver = PluginDriver::new(plugins);

        let common: CommonPluginContext = common_ctx();

        let ctx: PluginContext<'_> = make_plugin_ctx(&common);

        driver.compile_start(&ctx, &make_start_args()).await.unwrap();

        let recorded: Vec<String> = log.lock().unwrap().clone();

        assert_eq!(
            recorded,
            vec!["first:start".to_string(), "second:start".to_string(),],
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_start_error_names_plugin() {
        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(NotifyOrderPlugin {
                name: "first",
                stage: "start",
                log: Arc::new(Mutex::new(Vec::new())),
            }),
            Plugin::new_shared(FailingNotifyPlugin { hook: "compile_start" }),
        ];

        let driver: PluginDriver = PluginDriver::new(plugins);

        let common: CommonPluginContext = common_ctx();

        let ctx: PluginContext<'_> = make_plugin_ctx(&common);

        let err: anyhow::Error =
            driver.compile_start(&ctx, &make_start_args()).await.unwrap_err();

        assert!(
            format!("{err:#}").contains("`fail-start` compile_start"),
            "{err:#}"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_end_runs_all_plugins_after_first_error() {
        // compile_end runs for EVERY plugin even after one fails; the first
        // error is returned only after the loop, and the first plugin's
        // failure is the reported one (ordering: the loop never stops).
        let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(NotifyOrderPlugin {
                name: "first",
                stage: "end",
                log: Arc::clone(&log),
            }),
            Plugin::new_shared(FailingNotifyPlugin { hook: "compile_end" }),
            Plugin::new_shared(NotifyOrderPlugin {
                name: "third",
                stage: "end",
                log: Arc::clone(&log),
            }),
        ];

        let driver: PluginDriver = PluginDriver::new(plugins);

        let common: CommonPluginContext = common_ctx();

        let ctx: PluginContext<'_> = make_plugin_ctx(&common);

        let err: anyhow::Error =
            driver.compile_end(&ctx, &make_end_args()).await.unwrap_err();

        assert!(
            format!("{err:#}").contains("`fail-end` compile_end"),
            "{err:#}"
        );

        let recorded: Vec<String> = log.lock().unwrap().clone();

        assert_eq!(
            recorded,
            vec!["first:end".to_string(), "third:end".to_string()],
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_end_first_error_reported() {
        // Two failing compile_end plugins: the FIRST error (in registration
        // order) surfaces, and both hooks still run.
        let second_ran: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(FailingNotifyPluginAt {
                name: "fail-end-1",
                ran: Arc::new(Mutex::new(false)),
            }),
            Plugin::new_shared(FailingNotifyPluginAt {
                name: "fail-end-2",
                ran: Arc::clone(&second_ran),
            }),
        ];

        let driver: PluginDriver = PluginDriver::new(plugins);

        let common: CommonPluginContext = common_ctx();

        let ctx: PluginContext<'_> = make_plugin_ctx(&common);

        let err: anyhow::Error =
            driver.compile_end(&ctx, &make_end_args()).await.unwrap_err();

        assert!(
            format!("{err:#}").contains("`fail-end-1` compile_end"),
            "{err:#}"
        );

        assert!(*second_ran.lock().unwrap(), "second hook still ran");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_settled_names_preserves_duplicates() {
        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(MarkPlugin),
            Plugin::new_shared(MarkPlugin),
        ];

        let driver: PluginDriver = PluginDriver::new(plugins);

        assert_eq!(
            driver.settled_names(),
            vec![String::from("mark"), String::from("mark")],
        );
    }

    #[derive(Debug)]
    struct MetaOrderPlugin {
        name: &'static str,
        log: Arc<Mutex<Vec<String>>>,
        prepare_meta: Option<PluginHookMeta>,
        transform_meta: Option<PluginHookMeta>,
        finalize_meta: Option<PluginHookMeta>,
        compile_start_meta: Option<PluginHookMeta>,
        compile_end_meta: Option<PluginHookMeta>,
    }

    impl MetaOrderPlugin {
        fn new(
            name: &'static str,
            log: &Arc<Mutex<Vec<String>>>,
        ) -> Self {
            Self {
                name,
                log: Arc::clone(log),
                prepare_meta: None,
                transform_meta: None,
                finalize_meta: None,
                compile_start_meta: None,
                compile_end_meta: None,
            }
        }

        fn with_prepare_meta(
            mut self,
            order: PluginOrder,
        ) -> Self {
            self.prepare_meta = Some(PluginHookMeta { order: Some(order) });

            self
        }

        fn with_prepare_meta_none(mut self) -> Self {
            self.prepare_meta = Some(PluginHookMeta { order: None });

            self
        }

        fn with_transform_meta(
            mut self,
            order: PluginOrder,
        ) -> Self {
            self.transform_meta = Some(PluginHookMeta { order: Some(order) });

            self
        }

        fn with_finalize_meta(
            mut self,
            order: PluginOrder,
        ) -> Self {
            self.finalize_meta = Some(PluginHookMeta { order: Some(order) });

            self
        }

        fn with_compile_start_meta(
            mut self,
            order: PluginOrder,
        ) -> Self {
            self.compile_start_meta =
                Some(PluginHookMeta { order: Some(order) });

            self
        }

        fn with_compile_end_meta(
            mut self,
            order: PluginOrder,
        ) -> Self {
            self.compile_end_meta = Some(PluginHookMeta { order: Some(order) });

            self
        }

        fn record(
            &self,
            stage: &str,
        ) {
            let mut log: MutexGuard<'_, Vec<String>> = self.log.lock().unwrap();

            log.push([self.name.to_string(), stage.to_string()].join(":"));
        }
    }

    impl Plugin for MetaOrderPlugin {
        fn name(&self) -> Cow<'static, str> {
            self.name.into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Prepare | HookUsage::Transform | HookUsage::Finalize
        }

        fn prepare_meta(&self) -> Option<PluginHookMeta> {
            self.prepare_meta
        }

        fn transform_meta(&self) -> Option<PluginHookMeta> {
            self.transform_meta
        }

        fn finalize_meta(&self) -> Option<PluginHookMeta> {
            self.finalize_meta
        }

        fn compile_start_meta(&self) -> Option<PluginHookMeta> {
            self.compile_start_meta
        }

        fn compile_end_meta(&self) -> Option<PluginHookMeta> {
            self.compile_end_meta
        }

        fn prepare<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            _args: &'a PrepareArgs<'_>,
        ) -> impl Future<Output = PrepareReturn> + Send {
            async move {
                self.record("prepare");

                Ok(None)
            }
        }

        fn finalize<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            _args: &'a FinalizeArgs<'_>,
        ) -> impl Future<Output = FinalizeReturn> + Send {
            async move {
                self.record("finalize");

                Ok(None)
            }
        }

        fn transform<'a, 'ast: 'a>(
            &'a self,
            _ctx: &'a PluginContext<'a>,
            _args: TransformArgs<'ast>,
        ) -> impl Future<Output = TransformReturn<'ast>> + 'a {
            async move {
                self.record("transform");

                Ok(None)
            }
        }

        fn compile_start<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            _args: &'a crate::CompileStartArgs,
        ) -> impl Future<Output = NotifyReturn> + Send {
            async move {
                self.record("compile_start");

                Ok(())
            }
        }

        fn compile_end<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            _args: &'a crate::CompileEndArgs,
        ) -> impl Future<Output = NotifyReturn> + Send {
            async move {
                self.record("compile_end");

                Ok(())
            }
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_prepare_pre_normal_post_interleaving() {
        let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(MetaOrderPlugin::new("a", &log)),
            Plugin::new_shared(
                MetaOrderPlugin::new("b", &log)
                    .with_prepare_meta(PluginOrder::Pre),
            ),
            Plugin::new_shared(
                MetaOrderPlugin::new("c", &log)
                    .with_prepare_meta(PluginOrder::Post),
            ),
        ];

        let driver: PluginDriver = PluginDriver::new(plugins);

        let common: CommonPluginContext = common_ctx();

        let ctx: PluginContext<'_> = make_plugin_ctx(&common);

        let args: PrepareArgs<'_> = PrepareArgs { code: "console.log(1);" };

        driver.prepare(&ctx, &args).await.unwrap();

        let recorded: Vec<String> = log.lock().unwrap().clone();

        assert_eq!(
            recorded,
            vec![
                "b:prepare".to_string(),
                "a:prepare".to_string(),
                "c:prepare".to_string(),
            ],
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_prepare_stable_within_bucket() {
        let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(MetaOrderPlugin::new("normal", &log)),
            Plugin::new_shared(
                MetaOrderPlugin::new("pre-1", &log)
                    .with_prepare_meta(PluginOrder::Pre),
            ),
            Plugin::new_shared(
                MetaOrderPlugin::new("pre-2", &log)
                    .with_prepare_meta(PluginOrder::Pre),
            ),
            Plugin::new_shared(
                MetaOrderPlugin::new("post-1", &log)
                    .with_prepare_meta(PluginOrder::Post),
            ),
            Plugin::new_shared(
                MetaOrderPlugin::new("post-2", &log)
                    .with_prepare_meta(PluginOrder::Post),
            ),
        ];

        let driver: PluginDriver = PluginDriver::new(plugins);

        let common: CommonPluginContext = common_ctx();

        let ctx: PluginContext<'_> = make_plugin_ctx(&common);

        let args: PrepareArgs<'_> = PrepareArgs { code: "console.log(1);" };

        driver.prepare(&ctx, &args).await.unwrap();

        let recorded: Vec<String> = log.lock().unwrap().clone();

        assert_eq!(
            recorded,
            vec![
                "pre-1:prepare".to_string(),
                "pre-2:prepare".to_string(),
                "normal:prepare".to_string(),
                "post-1:prepare".to_string(),
                "post-2:prepare".to_string(),
            ],
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_meta_is_per_hook() {
        // `meta` is `Pre` on prepare but normal on finalize: it leads the
        // prepare fold yet trails `plain` for finalize.
        let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(MetaOrderPlugin::new("plain", &log)),
            Plugin::new_shared(
                MetaOrderPlugin::new("meta", &log)
                    .with_prepare_meta(PluginOrder::Pre),
            ),
        ];

        let driver: PluginDriver = PluginDriver::new(plugins);

        let common: CommonPluginContext = common_ctx();

        let ctx: PluginContext<'_> = make_plugin_ctx(&common);

        let args: PrepareArgs<'_> = PrepareArgs { code: "console.log(1);" };

        driver.prepare(&ctx, &args).await.unwrap();

        assert_eq!(
            log.lock().unwrap().clone(),
            vec!["meta:prepare".to_string(), "plain:prepare".to_string()],
        );

        log.lock().unwrap().clear();

        let finalize_args: FinalizeArgs<'_> =
            FinalizeArgs { code: "console.log(1);" };

        driver.finalize(&ctx, &finalize_args).await.unwrap();

        assert_eq!(
            log.lock().unwrap().clone(),
            vec!["plain:finalize".to_string(), "meta:finalize".to_string()],
        );
    }

    #[test]
    fn test_sort_plugins_by_hook_meta_buckets_and_stability() {
        // Scrambled registration: normal, pre, post, pre, normal, post.
        // Ranking must yield [pre, pre, normal, normal, post, post], stable.
        let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(MetaOrderPlugin::new("n1", &log)),
            Plugin::new_shared(
                MetaOrderPlugin::new("pre1", &log)
                    .with_prepare_meta(PluginOrder::Pre),
            ),
            Plugin::new_shared(
                MetaOrderPlugin::new("post1", &log)
                    .with_prepare_meta(PluginOrder::Post),
            ),
            Plugin::new_shared(
                MetaOrderPlugin::new("pre2", &log)
                    .with_prepare_meta(PluginOrder::Pre),
            ),
            Plugin::new_shared(MetaOrderPlugin::new("n2", &log)),
            Plugin::new_shared(
                MetaOrderPlugin::new("post2", &log)
                    .with_prepare_meta(PluginOrder::Post),
            ),
        ];

        let ranked: Vec<SharedPluginable> =
            sort_plugins_by_hook_meta(&plugins, |plugin| {
                plugin.call_prepare_meta()
            });

        let names: Vec<String> = ranked
            .iter()
            .map(|plugin| plugin.call_name().into_owned())
            .collect();

        assert_eq!(
            names,
            vec![
                "pre1".to_string(),
                "pre2".to_string(),
                "n1".to_string(),
                "n2".to_string(),
                "post1".to_string(),
                "post2".to_string(),
            ],
        );
    }

    #[test]
    fn test_sort_plugins_by_hook_meta_empty_and_all_normal() {
        let empty: Vec<SharedPluginable> =
            sort_plugins_by_hook_meta(&[], |plugin| plugin.call_prepare_meta());

        assert!(empty.is_empty());

        let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(MetaOrderPlugin::new("a", &log)),
            Plugin::new_shared(MetaOrderPlugin::new("b", &log)),
        ];

        let ranked: Vec<SharedPluginable> =
            sort_plugins_by_hook_meta(&plugins, |plugin| {
                plugin.call_prepare_meta()
            });

        let names: Vec<String> = ranked
            .iter()
            .map(|plugin| plugin.call_name().into_owned())
            .collect();

        assert_eq!(names, vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn test_sort_plugins_by_hook_meta_meta_without_order_is_normal() {
        // `Some(PluginHookMeta { order: None })` must land in the normal
        // bucket: a present meta without an order is not a promotion.
        let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(
                MetaOrderPlugin::new("pre", &log)
                    .with_prepare_meta(PluginOrder::Pre),
            ),
            Plugin::new_shared(
                MetaOrderPlugin::new("meta-none", &log)
                    .with_prepare_meta_none(),
            ),
            Plugin::new_shared(MetaOrderPlugin::new("plain", &log)),
        ];

        let ranked: Vec<SharedPluginable> =
            sort_plugins_by_hook_meta(&plugins, |plugin| {
                plugin.call_prepare_meta()
            });

        let names: Vec<String> = ranked
            .iter()
            .map(|plugin| plugin.call_name().into_owned())
            .collect();

        assert_eq!(
            names,
            vec![
                "pre".to_string(),
                "meta-none".to_string(),
                "plain".to_string(),
            ],
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_transform_pre_normal_post_interleaving() {
        let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(MetaOrderPlugin::new("a", &log)),
            Plugin::new_shared(
                MetaOrderPlugin::new("b", &log)
                    .with_transform_meta(PluginOrder::Pre),
            ),
            Plugin::new_shared(
                MetaOrderPlugin::new("c", &log)
                    .with_transform_meta(PluginOrder::Post),
            ),
        ];

        let driver: PluginDriver = PluginDriver::new(plugins);

        let common: CommonPluginContext = common_ctx();

        let ctx: PluginContext<'_> = make_plugin_ctx(&common);

        let allocator: Allocator = Allocator::default();

        let parsed: ParseResult<'_> = test_parse_program(&allocator);

        let args: TransformArgs<'_> =
            TransformArgs { allocator: &allocator, ast: &parsed.program };

        driver.transform(&ctx, &args).await.unwrap();

        assert_eq!(
            log.lock().unwrap().clone(),
            vec![
                "b:transform".to_string(),
                "a:transform".to_string(),
                "c:transform".to_string(),
            ],
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_finalize_pre_normal_post_interleaving() {
        let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(MetaOrderPlugin::new("a", &log)),
            Plugin::new_shared(
                MetaOrderPlugin::new("b", &log)
                    .with_finalize_meta(PluginOrder::Pre),
            ),
            Plugin::new_shared(
                MetaOrderPlugin::new("c", &log)
                    .with_finalize_meta(PluginOrder::Post),
            ),
        ];

        let driver: PluginDriver = PluginDriver::new(plugins);

        let common: CommonPluginContext = common_ctx();

        let ctx: PluginContext<'_> = make_plugin_ctx(&common);

        let args: FinalizeArgs<'_> = FinalizeArgs { code: "console.log(1);" };

        driver.finalize(&ctx, &args).await.unwrap();

        assert_eq!(
            log.lock().unwrap().clone(),
            vec![
                "b:finalize".to_string(),
                "a:finalize".to_string(),
                "c:finalize".to_string(),
            ],
        );
    }

    #[derive(Debug)]
    struct OptionsMetaNotifyPlugin {
        name: &'static str,
        options_meta: Option<PluginHookMeta>,
        log: Arc<Mutex<Vec<String>>>,
    }

    impl Plugin for OptionsMetaNotifyPlugin {
        fn name(&self) -> Cow<'static, str> {
            self.name.into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::CompileStart
        }

        fn options_meta(&self) -> Option<PluginHookMeta> {
            self.options_meta
        }

        fn compile_start<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            _args: &'a crate::CompileStartArgs,
        ) -> impl Future<Output = NotifyReturn> + Send {
            async move {
                self.log.lock().unwrap().push(self.name.to_string());

                Ok(())
            }
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_options_meta_does_not_leak_into_compile_start_order() {
        // A plugin with `options: Pre` must NOT be promoted in the
        // compile_start bucket: options meta is per-hook and the driver only
        // reads compile_start meta. `pre-options` still trails the normal
        // plugin in registration order.
        let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(OptionsMetaNotifyPlugin {
                name: "normal",
                options_meta: None,
                log: Arc::clone(&log),
            }),
            Plugin::new_shared(OptionsMetaNotifyPlugin {
                name: "pre-options",
                options_meta: Some(PluginHookMeta {
                    order: Some(PluginOrder::Pre),
                }),
                log: Arc::clone(&log),
            }),
        ];

        let driver: PluginDriver = PluginDriver::new(plugins);

        let common: CommonPluginContext = common_ctx();

        let ctx: PluginContext<'_> = make_plugin_ctx(&common);

        driver.compile_start(&ctx, &make_start_args()).await.unwrap();

        assert_eq!(
            log.lock().unwrap().clone(),
            vec!["normal".to_string(), "pre-options".to_string()],
        );

        assert_eq!(
            driver.settled_names(),
            vec!["normal".to_string(), "pre-options".to_string()],
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_notify_hooks_ordered_while_settled_names_stay_registration() {
        let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(MetaOrderPlugin::new("a", &log)),
            Plugin::new_shared(
                MetaOrderPlugin::new("b", &log)
                    .with_compile_start_meta(PluginOrder::Pre)
                    .with_compile_end_meta(PluginOrder::Pre),
            ),
            Plugin::new_shared(
                MetaOrderPlugin::new("c", &log)
                    .with_compile_start_meta(PluginOrder::Post)
                    .with_compile_end_meta(PluginOrder::Post),
            ),
        ];

        let driver: PluginDriver = PluginDriver::new(plugins);

        let common: CommonPluginContext = common_ctx();

        let ctx: PluginContext<'_> = make_plugin_ctx(&common);

        driver.compile_start(&ctx, &make_start_args()).await.unwrap();

        assert_eq!(
            log.lock().unwrap().clone(),
            vec![
                "b:compile_start".to_string(),
                "a:compile_start".to_string(),
                "c:compile_start".to_string(),
            ],
        );

        log.lock().unwrap().clear();

        driver.compile_end(&ctx, &make_end_args()).await.unwrap();

        assert_eq!(
            log.lock().unwrap().clone(),
            vec![
                "b:compile_end".to_string(),
                "a:compile_end".to_string(),
                "c:compile_end".to_string(),
            ],
        );

        assert_eq!(
            driver.settled_names(),
            vec!["a".to_string(), "b".to_string(), "c".to_string()],
        );
    }
}
