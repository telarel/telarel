pub mod hooks;

use std::sync::Arc;

use telarel_common::HookUsage;

use crate::_types::context::PluginContext;
use crate::_types::hooks::compile_end::CompileEndArgs;
use crate::_types::hooks::compile_start::CompileStartArgs;
use crate::_types::hooks::post::PostArgs;
use crate::_types::hooks::pre::PreArgs;
use crate::_types::hooks::transform::{TransformArgs, TransformOutput};
use crate::plugin::pluginable::SharedPluginable;

/// Drives plugins through the hooks in registration order.
pub struct PluginDriver {
    /// The full settled plugin list; `compile_start`/`compile_end` iterate it.
    all_plugins: Vec<SharedPluginable>,
    usage: HookUsage,
    pre_plugins: Vec<SharedPluginable>,
    transform_plugins: Vec<SharedPluginable>,
    post_plugins: Vec<SharedPluginable>,
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

        let mut pre_plugins: Vec<SharedPluginable> = Vec::new();
        let mut transform_plugins: Vec<SharedPluginable> = Vec::new();
        let mut post_plugins: Vec<SharedPluginable> = Vec::new();

        for plugin in &plugins {
            let declared: HookUsage = plugin.call_register_hook_usage();

            usage |= declared;

            if declared.contains(HookUsage::Pre) {
                pre_plugins.push(Arc::clone(plugin));
            }

            if declared.contains(HookUsage::Transform) {
                transform_plugins.push(Arc::clone(plugin));
            }

            if declared.contains(HookUsage::Post) {
                post_plugins.push(Arc::clone(plugin));
            }
        }

        Self {
            all_plugins: plugins,
            usage,
            pre_plugins,
            transform_plugins,
            post_plugins,
        }
    }

    /// The aggregate hook usage across all plugins.
    pub fn usage(&self) -> HookUsage {
        self.usage
    }

    /// The settled plugin names in registration order; duplicates preserved.
    pub fn settled_names(&self) -> Vec<String> {
        self.all_plugins
            .iter()
            .map(|plugin| plugin.call_name().into_owned())
            .collect()
    }

    /// Run the `compile_start` hook on EVERY settled plugin (not partitioned
    /// by usage); the first error aborts. Notify-only.
    pub async fn compile_start(
        &self,
        ctx: &PluginContext<'_>,
        args: &CompileStartArgs,
    ) -> anyhow::Result<()> {
        hooks::compile_start::compile_start(&self.all_plugins, ctx, args).await
    }

    /// Run the `pre` hook on every plugin in registration order;
    /// `Some` replaces the carried code, `None` keeps it. The fold result
    /// carries every returned map in fold order so the caller composes the
    /// full chain (see [`hooks::pre::PreFold`]).
    pub async fn pre(
        &self,
        ctx: &PluginContext<'_>,
        args: &PreArgs<'_>,
    ) -> anyhow::Result<hooks::pre::PreFold> {
        hooks::pre::pre(&self.pre_plugins, ctx, args).await
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

    /// Run the `post` hook on every plugin in registration order;
    /// `Some` replaces the carried code, `None` keeps it. The fold result
    /// carries every returned map in fold order so the caller composes the
    /// full chain (see [`hooks::post::PostFold`]).
    pub async fn post(
        &self,
        ctx: &PluginContext<'_>,
        args: &PostArgs<'_>,
    ) -> anyhow::Result<hooks::post::PostFold> {
        hooks::post::post(&self.post_plugins, ctx, args).await
    }

    /// Run the `compile_end` hook on EVERY settled plugin (not partitioned by
    /// usage); every hook runs even after one fails, and the first error is
    /// returned after the loop. Notify-only.
    pub async fn compile_end(
        &self,
        ctx: &PluginContext<'_>,
        args: &CompileEndArgs,
    ) -> anyhow::Result<()> {
        hooks::compile_end::compile_end(&self.all_plugins, ctx, args).await
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
    use crate::_types::hooks::notify::NotifyReturn;
    use crate::_types::hooks::post::PostReturn;
    use crate::_types::hooks::pre::PreReturn;
    use crate::_types::hooks::transform::{TransformArgs, TransformReturn};
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
            HookUsage::Pre | HookUsage::Post
        }

        fn pre<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            _args: &'a PreArgs<'_>,
        ) -> impl Future<Output = PreReturn> + Send {
            async move {
                self.record();

                Ok(None)
            }
        }

        fn post<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            _args: &'a PostArgs<'_>,
        ) -> impl Future<Output = PostReturn> + Send {
            async move {
                self.record();

                Ok(None)
            }
        }
    }

    #[derive(Debug)]
    struct FailingPrePlugin;

    impl Plugin for FailingPrePlugin {
        fn name(&self) -> Cow<'static, str> {
            "fail-pre".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Pre
        }

        fn pre<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            _args: &'a PreArgs<'_>,
        ) -> impl Future<Output = PreReturn> + Send {
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
    struct FailingPostPlugin;

    impl Plugin for FailingPostPlugin {
        fn name(&self) -> Cow<'static, str> {
            "fail-post".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Post
        }

        fn post(
            &self,
            _ctx: &PluginContext<'_>,
            _args: &PostArgs<'_>,
        ) -> impl Future<Output = PostReturn> + Send {
            async move { Err(anyhow::anyhow!("boom")) }
        }
    }

    #[derive(Debug)]
    struct PreOnlyPlugin {
        name: &'static str,
        log: Arc<Mutex<Vec<String>>>,
    }

    impl PreOnlyPlugin {
        fn new(log: Arc<Mutex<Vec<String>>>) -> Self {
            Self { name: "pre-only", log }
        }

        fn record(&self) {
            let mut log: MutexGuard<'_, Vec<String>> = self.log.lock().unwrap();
            log.push(self.name.to_string());
        }
    }

    impl Plugin for PreOnlyPlugin {
        fn name(&self) -> Cow<'static, str> {
            self.name.into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Pre
        }

        fn pre<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            _args: &'a PreArgs<'_>,
        ) -> impl Future<Output = PreReturn> + Send {
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
    async fn test_pre_runs_all_plugins_in_order() {
        let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(OrderPlugin::new("first", Arc::clone(&log))),
            Plugin::new_shared(OrderPlugin::new("second", Arc::clone(&log))),
        ];

        let driver: PluginDriver = PluginDriver::new(plugins);

        let common: CommonPluginContext = common_ctx();

        let ctx: PluginContext<'_> = make_plugin_ctx(&common);

        let args: PreArgs<'_> = PreArgs { code: "console.log(1);" };

        let fold: hooks::pre::PreFold = driver.pre(&ctx, &args).await.unwrap();

        assert!(fold.output.is_none());

        assert!(fold.maps.is_empty());

        let recorded: Vec<String> = log.lock().unwrap().clone();

        assert_eq!(recorded, vec!["first".to_string(), "second".to_string()]);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_pre_error_aborts_and_names_plugin() {
        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(OrderPlugin::new(
                "first",
                Arc::new(Mutex::new(Vec::new())),
            )),
            Plugin::new_shared(FailingPrePlugin),
        ];

        let driver: PluginDriver = PluginDriver::new(plugins);

        let common: CommonPluginContext = common_ctx();

        let ctx: PluginContext<'_> = make_plugin_ctx(&common);

        let args: PreArgs<'_> = PreArgs { code: "console.log(1);" };

        let err: anyhow::Error = driver.pre(&ctx, &args).await.unwrap_err();

        assert!(format!("{err:#}").contains("`fail-pre` pre"), "{err:#}");
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
    async fn test_post_runs_all_plugins_in_order() {
        let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(OrderPlugin::new("first", Arc::clone(&log))),
            Plugin::new_shared(OrderPlugin::new("second", Arc::clone(&log))),
        ];

        let driver: PluginDriver = PluginDriver::new(plugins);

        let common: CommonPluginContext = common_ctx();

        let ctx: PluginContext<'_> = make_plugin_ctx(&common);

        let args: PostArgs<'_> = PostArgs { code: "console.log(1);" };

        let fold: hooks::post::PostFold =
            driver.post(&ctx, &args).await.unwrap();

        assert!(fold.output.is_none());

        assert!(fold.maps.is_empty());

        let recorded: Vec<String> = log.lock().unwrap().clone();

        assert_eq!(recorded, vec!["first".to_string(), "second".to_string()]);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_post_error_names_plugin() {
        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(OrderPlugin::new(
                "first",
                Arc::new(Mutex::new(Vec::new())),
            )),
            Plugin::new_shared(FailingPostPlugin),
        ];

        let driver: PluginDriver = PluginDriver::new(plugins);

        let common: CommonPluginContext = common_ctx();

        let ctx: PluginContext<'_> = make_plugin_ctx(&common);

        let args: PostArgs<'_> = PostArgs { code: "console.log(1);" };

        let err: anyhow::Error = driver.post(&ctx, &args).await.unwrap_err();

        assert!(format!("{err:#}").contains("`fail-post` post"), "{err:#}");
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
            vec![Plugin::new_shared(PreOnlyPlugin::new(Arc::clone(&log)))];

        let driver: PluginDriver = PluginDriver::new(plugins);

        driver.transform(&ctx, &args).await.unwrap();

        let recorded: Vec<String> = log.lock().unwrap().clone();

        assert_eq!(driver.usage(), HookUsage::Pre);
        assert!(recorded.is_empty());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_driver_aggregates_usage() {
        // An options-only plugin still contributes its declared bit to the
        // aggregate (no options partition exists — the fixpoint runner owns
        // the options stage).
        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(PreOnlyPlugin::new(Arc::new(Mutex::new(
                Vec::new(),
            )))),
            Plugin::new_shared(FailingPrePlugin),
            Plugin::new_shared(MarkPlugin),
            Plugin::new_shared(FailingPostPlugin),
        ];

        let driver: PluginDriver = PluginDriver::new(plugins);

        assert_eq!(
            driver.usage(),
            HookUsage::Pre | HookUsage::Transform | HookUsage::Post
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
            HookUsage::Pre
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
        // OrderPlugin declares Pre|Post only, yet its compile_start must
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
}
