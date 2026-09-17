pub mod hooks;

use std::sync::Arc;

use oxc::ast::ast::Program;
use telarel_common::{CompileContext, CompileOptions, HookUsage};

use crate::_types::hooks::post::PostArgs;
use crate::_types::hooks::pre::PreArgs;
use crate::_types::hooks::transform::TransformArgs;
use crate::plugin::pluginable::SharedPluginable;

/// Drives plugins through the hooks in registration order.
pub struct PluginDriver {
    usage: HookUsage,
    options_plugins: Vec<SharedPluginable>,
    pre_plugins: Vec<SharedPluginable>,
    transform_plugins: Vec<SharedPluginable>,
    post_plugins: Vec<SharedPluginable>,
}

impl PluginDriver {
    /// Create a driver from plugins in registration order.
    ///
    /// Plugins are partitioned by their declared [`HookUsage`]; a hook only
    /// iterates the plugins that declared it.
    pub fn new(plugins: Vec<SharedPluginable>) -> Self {
        let mut usage: HookUsage = HookUsage::default();

        let mut options_plugins: Vec<SharedPluginable> = Vec::new();
        let mut pre_plugins: Vec<SharedPluginable> = Vec::new();
        let mut transform_plugins: Vec<SharedPluginable> = Vec::new();
        let mut post_plugins: Vec<SharedPluginable> = Vec::new();

        for plugin in &plugins {
            let declared: HookUsage = plugin.call_hook_usage();

            usage |= declared;

            if declared.contains(HookUsage::OPTIONS) {
                options_plugins.push(Arc::clone(plugin));
            }

            if declared.contains(HookUsage::PRE) {
                pre_plugins.push(Arc::clone(plugin));
            }

            if declared.contains(HookUsage::TRANSFORM) {
                transform_plugins.push(Arc::clone(plugin));
            }

            if declared.contains(HookUsage::POST) {
                post_plugins.push(Arc::clone(plugin));
            }
        }

        Self {
            usage,
            options_plugins,
            pre_plugins,
            transform_plugins,
            post_plugins,
        }
    }

    /// The aggregate hook usage across all plugins.
    pub fn usage(&self) -> HookUsage {
        self.usage
    }

    /// Run the `options` hook chain; the last `Some` output wins.
    pub async fn options(
        &self,
        options: CompileOptions,
    ) -> anyhow::Result<CompileOptions> {
        hooks::options::options(&self.options_plugins, options).await
    }

    /// Run the `pre` hook on every plugin in registration order.
    pub async fn pre(
        &self,
        ctx: &CompileContext<'_>,
        args: &PreArgs<'_>,
    ) -> anyhow::Result<()> {
        hooks::pre::pre(&self.pre_plugins, ctx, args).await
    }

    /// Run the `transform` hook chain; returns the final replaced
    /// [`Program`], or `None` when no plugin replaced.
    pub async fn transform<'a>(
        &'a self,
        ctx: &'a CompileContext<'a>,
        args: &'a TransformArgs<'a>,
    ) -> anyhow::Result<Option<Program<'a>>> {
        hooks::transform::transform(&self.transform_plugins, ctx, args).await
    }

    /// Run the `post` hook on every plugin in registration order.
    pub async fn post(
        &self,
        ctx: &CompileContext<'_>,
        args: &PostArgs<'_>,
    ) -> anyhow::Result<()> {
        hooks::post::post(&self.post_plugins, ctx, args).await
    }
}

// The frozen driver test keeps the explicit RPITIT shape for `MarkPlugin::
// transform`; converting it to `async fn` would be a second edit to the
// frozen file, so the `manual_async_fn` lint is suppressed here instead.
#[cfg(test)]
#[allow(clippy::manual_async_fn)]
mod tests {
    use std::borrow::Cow;
    use std::future::Future;
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::sync::MutexGuard;

    use telarel_common::{
        CompileContext, CompileOptions, HookUsage, ParseOptions, ParseResult,
        PartialCompileOptions, parse,
    };

    use oxc::allocator::Allocator;
    use oxc::ast::ast::Program;

    use crate::_types::hooks::transform::{
        TransformArgs, TransformOutput, TransformReturn,
    };
    use crate::SharedPluginable;
    use crate::plugin::Plugin;
    use crate::plugin::pluginable::Pluginable;

    use super::*;

    #[derive(Debug)]
    struct OptionsPlugin;

    impl Plugin for OptionsPlugin {
        fn name(&self) -> Cow<'static, str> {
            "options".into()
        }

        fn hook_usage(&self) -> HookUsage {
            HookUsage::OPTIONS
        }

        async fn options(
            &self,
            _options: &CompileOptions,
        ) -> anyhow::Result<Option<PartialCompileOptions>> {
            Ok(Some(PartialCompileOptions {
                cwd: Some("/changed".to_string()),
                ..PartialCompileOptions::default()
            }))
        }
    }

    #[derive(Debug)]
    struct NoopOptionsPlugin;

    impl Plugin for NoopOptionsPlugin {
        fn name(&self) -> Cow<'static, str> {
            "noop-options".into()
        }

        fn hook_usage(&self) -> HookUsage {
            HookUsage::OPTIONS
        }
    }

    #[derive(Debug)]
    struct PartialOptionsPlugin;

    impl Plugin for PartialOptionsPlugin {
        fn name(&self) -> Cow<'static, str> {
            "partial-options".into()
        }

        fn hook_usage(&self) -> HookUsage {
            HookUsage::OPTIONS
        }

        async fn options(
            &self,
            _options: &CompileOptions,
        ) -> anyhow::Result<Option<PartialCompileOptions>> {
            Ok(Some(PartialCompileOptions {
                code: Some("const rewritten = 7;".to_string()),
                ..PartialCompileOptions::default()
            }))
        }
    }

    #[derive(Debug)]
    struct ObserveOptionsPlugin {
        observed: Arc<Mutex<Vec<String>>>,
    }

    impl ObserveOptionsPlugin {
        fn new(observed: Arc<Mutex<Vec<String>>>) -> Self {
            Self { observed }
        }
    }

    impl Plugin for ObserveOptionsPlugin {
        fn name(&self) -> Cow<'static, str> {
            "observe-options".into()
        }

        fn hook_usage(&self) -> HookUsage {
            HookUsage::OPTIONS
        }

        async fn options(
            &self,
            options: &CompileOptions,
        ) -> anyhow::Result<Option<PartialCompileOptions>> {
            self.observed.lock().unwrap().push(options.code.clone());

            Ok(None)
        }
    }

    #[derive(Debug)]
    struct FailingOptionsPlugin;

    impl Plugin for FailingOptionsPlugin {
        fn name(&self) -> Cow<'static, str> {
            "fail-options".into()
        }

        fn hook_usage(&self) -> HookUsage {
            HookUsage::OPTIONS
        }

        async fn options(
            &self,
            _options: &CompileOptions,
        ) -> anyhow::Result<Option<PartialCompileOptions>> {
            Err(anyhow::anyhow!("boom"))
        }
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

        fn hook_usage(&self) -> HookUsage {
            HookUsage::PRE | HookUsage::POST
        }

        async fn pre(
            &self,
            _ctx: &CompileContext<'_>,
            _args: &PreArgs<'_>,
        ) -> anyhow::Result<()> {
            self.record();
            Ok(())
        }

        async fn post(
            &self,
            _ctx: &CompileContext<'_>,
            _args: &PostArgs<'_>,
        ) -> anyhow::Result<()> {
            self.record();
            Ok(())
        }
    }

    #[derive(Debug)]
    struct FailingPrePlugin;

    impl Plugin for FailingPrePlugin {
        fn name(&self) -> Cow<'static, str> {
            "fail-pre".into()
        }

        fn hook_usage(&self) -> HookUsage {
            HookUsage::PRE
        }

        async fn pre(
            &self,
            _ctx: &CompileContext<'_>,
            _args: &PreArgs<'_>,
        ) -> anyhow::Result<()> {
            Err(anyhow::anyhow!("boom"))
        }
    }

    #[derive(Debug)]
    struct MarkPlugin;

    impl Plugin for MarkPlugin {
        fn name(&self) -> Cow<'static, str> {
            "mark".into()
        }

        fn hook_usage(&self) -> HookUsage {
            HookUsage::TRANSFORM
        }

        fn transform<'a>(
            &'a self,
            ctx: &'a CompileContext<'a>,
            args: &'a TransformArgs<'a>,
        ) -> impl Future<Output = crate::_types::hooks::transform::TransformReturn<'a>>
        {
            async move {
                let code: &'a str = args.allocator.alloc_str("\"mark\";");
                let options: ParseOptions<'_, '_> = ParseOptions {
                    context: ctx,
                    allocator: args.allocator,
                    file: args.file,
                    code,
                };
                let parsed: ParseResult<'_> = parse(options).unwrap();
                Ok(Some(TransformOutput { program: parsed.program }))
            }
        }
    }

    #[derive(Debug)]
    struct FailingTransformPlugin;

    impl Plugin for FailingTransformPlugin {
        fn name(&self) -> Cow<'static, str> {
            "fail-transform".into()
        }

        fn hook_usage(&self) -> HookUsage {
            HookUsage::TRANSFORM
        }

        fn transform<'a>(
            &'a self,
            _ctx: &'a CompileContext<'a>,
            _args: &'a TransformArgs<'a>,
        ) -> impl Future<Output = crate::_types::hooks::transform::TransformReturn<'a>>
        {
            async move { Err(anyhow::anyhow!("boom")) }
        }
    }

    #[derive(Debug)]
    struct FailingPostPlugin;

    impl Plugin for FailingPostPlugin {
        fn name(&self) -> Cow<'static, str> {
            "fail-post".into()
        }

        fn hook_usage(&self) -> HookUsage {
            HookUsage::POST
        }

        async fn post(
            &self,
            _ctx: &CompileContext<'_>,
            _args: &PostArgs<'_>,
        ) -> anyhow::Result<()> {
            Err(anyhow::anyhow!("boom"))
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

        fn hook_usage(&self) -> HookUsage {
            HookUsage::PRE
        }

        async fn pre(
            &self,
            _ctx: &CompileContext<'_>,
            _args: &PreArgs<'_>,
        ) -> anyhow::Result<()> {
            self.record();
            Ok(())
        }

        fn transform<'a>(
            &'a self,
            _ctx: &'a CompileContext<'a>,
            _args: &'a TransformArgs<'a>,
        ) -> impl Future<Output = TransformReturn<'a>> {
            async move {
                self.record();
                Ok(None)
            }
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_options_chain_last_wins() {
        let plugins: Vec<SharedPluginable> =
            vec![Arc::new(OptionsPlugin), Arc::new(OptionsPlugin)];

        let driver: PluginDriver = PluginDriver::new(plugins);

        let options: CompileOptions = CompileOptions {
            cwd: "/repo".to_string(),
            file: "a.ts".to_string(),
            code: "console.log(1);".to_string(),
        };

        let resolved: CompileOptions = driver.options(options).await.unwrap();

        assert_eq!(resolved.cwd, "/changed");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_options_none_keeps_current() {
        let plugins: Vec<SharedPluginable> = vec![Arc::new(NoopOptionsPlugin)];

        let driver: PluginDriver = PluginDriver::new(plugins);

        let options: CompileOptions = CompileOptions {
            cwd: "/repo".to_string(),
            file: "a.ts".to_string(),
            code: "console.log(1);".to_string(),
        };

        let resolved: CompileOptions = driver.options(options).await.unwrap();

        assert_eq!(resolved.cwd, "/repo");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_options_partial_merge_keeps_other_fields() {
        let plugins: Vec<SharedPluginable> =
            vec![Arc::new(PartialOptionsPlugin)];

        let driver: PluginDriver = PluginDriver::new(plugins);

        let options: CompileOptions = CompileOptions {
            cwd: "/repo".to_string(),
            file: "a.ts".to_string(),
            code: "console.log(1);".to_string(),
        };

        let resolved: CompileOptions = driver.options(options).await.unwrap();

        assert_eq!(resolved.code, "const rewritten = 7;");
        assert_eq!(resolved.cwd, "/repo");
        assert_eq!(resolved.file, "a.ts");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_options_partial_merge_visible_to_later_plugin() {
        let observed: Arc<Mutex<Vec<String>>> =
            Arc::new(Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> = vec![
            Arc::new(PartialOptionsPlugin),
            Arc::new(ObserveOptionsPlugin::new(Arc::clone(&observed))),
        ];

        let driver: PluginDriver = PluginDriver::new(plugins);

        let options: CompileOptions = CompileOptions {
            cwd: "/repo".to_string(),
            file: "a.ts".to_string(),
            code: "console.log(1);".to_string(),
        };

        let resolved: CompileOptions = driver.options(options).await.unwrap();

        assert_eq!(resolved.code, "const rewritten = 7;");

        let recorded: Vec<String> = observed.lock().unwrap().clone();

        assert_eq!(recorded, vec!["const rewritten = 7;".to_string()],);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_options_error_propagates() {
        let plugins: Vec<SharedPluginable> =
            vec![Arc::new(FailingOptionsPlugin)];

        let driver: PluginDriver = PluginDriver::new(plugins);

        let options: CompileOptions = CompileOptions {
            cwd: "/repo".to_string(),
            file: "a.ts".to_string(),
            code: "console.log(1);".to_string(),
        };

        let result: anyhow::Result<CompileOptions> =
            driver.options(options).await;

        assert!(result.is_err());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_pre_runs_all_plugins_in_order() {
        let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> = vec![
            Arc::new(OrderPlugin::new("first", Arc::clone(&log))),
            Arc::new(OrderPlugin::new("second", Arc::clone(&log))),
        ];

        let driver: PluginDriver = PluginDriver::new(plugins);

        let ctx: CompileContext<'_> =
            CompileContext::new("/repo", "a.ts", "console.log(1);");

        let args: PreArgs<'_> =
            PreArgs { file: "a.ts", code: "console.log(1);" };

        driver.pre(&ctx, &args).await.unwrap();

        let recorded: Vec<String> = log.lock().unwrap().clone();

        assert_eq!(recorded, vec!["first".to_string(), "second".to_string()]);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_pre_error_aborts_and_names_plugin() {
        let plugins: Vec<SharedPluginable> = vec![
            Arc::new(OrderPlugin::new(
                "first",
                Arc::new(Mutex::new(Vec::new())),
            )),
            Arc::new(FailingPrePlugin),
        ];

        let driver: PluginDriver = PluginDriver::new(plugins);

        let ctx: CompileContext<'_> =
            CompileContext::new("/repo", "a.ts", "console.log(1);");

        let args: PreArgs<'_> =
            PreArgs { file: "a.ts", code: "console.log(1);" };

        let err: anyhow::Error = driver.pre(&ctx, &args).await.unwrap_err();

        assert!(format!("{err:#}").contains("`fail-pre` pre"), "{err:#}");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_transform_chains_programs() {
        let allocator: Allocator = Allocator::default();

        let ctx: CompileContext<'_> =
            CompileContext::new("/repo", "a.ts", "console.log(1);");

        let options: ParseOptions<'_, '_> = ParseOptions {
            context: &ctx,
            allocator: &allocator,
            file: "a.ts",
            code: "console.log(1);",
        };

        let parsed: ParseResult<'_> = parse(options).unwrap();

        let program: Program<'_> = parsed.program;

        let args: TransformArgs<'_> = TransformArgs {
            allocator: &allocator,
            file: "a.ts",
            program: &program,
        };

        let plugins: Vec<SharedPluginable> =
            vec![Arc::new(MarkPlugin), Arc::new(MarkPlugin)];

        let driver: PluginDriver = PluginDriver::new(plugins);

        let replaced: Option<Program<'_>> =
            driver.transform(&ctx, &args).await.unwrap();

        let replaced: Program<'_> = replaced.expect("transform ran");

        // final program survives in the allocator; assert via codegen
        let out: String = oxc::codegen::Codegen::new().build(&replaced).code;

        assert!(out.contains("mark"), "{out}");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_transform_error_names_plugin() {
        let allocator: Allocator = Allocator::default();

        let ctx: CompileContext<'_> =
            CompileContext::new("/repo", "a.ts", "console.log(1);");

        let options: ParseOptions<'_, '_> = ParseOptions {
            context: &ctx,
            allocator: &allocator,
            file: "a.ts",
            code: "console.log(1);",
        };

        let parsed: ParseResult<'_> = parse(options).unwrap();

        let program: Program<'_> = parsed.program;

        let args: TransformArgs<'_> = TransformArgs {
            allocator: &allocator,
            file: "a.ts",
            program: &program,
        };

        let plugins: Vec<SharedPluginable> =
            vec![Arc::new(FailingTransformPlugin)];

        let driver: PluginDriver = PluginDriver::new(plugins);

        let err: anyhow::Error =
            driver.transform(&ctx, &args).await.unwrap_err();

        assert!(
            format!("{err:#}").contains("`fail-transform` transform"),
            "{err:#}"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_post_runs_all_plugins_in_order() {
        let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> = vec![
            Arc::new(OrderPlugin::new("first", Arc::clone(&log))),
            Arc::new(OrderPlugin::new("second", Arc::clone(&log))),
        ];

        let driver: PluginDriver = PluginDriver::new(plugins);

        let ctx: CompileContext<'_> =
            CompileContext::new("/repo", "a.ts", "console.log(1);");

        let args: PostArgs<'_> =
            PostArgs { file: "a.ts", code: "console.log(1);" };

        driver.post(&ctx, &args).await.unwrap();

        let recorded: Vec<String> = log.lock().unwrap().clone();

        assert_eq!(recorded, vec!["first".to_string(), "second".to_string()]);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_post_error_names_plugin() {
        let plugins: Vec<SharedPluginable> = vec![
            Arc::new(OrderPlugin::new(
                "first",
                Arc::new(Mutex::new(Vec::new())),
            )),
            Arc::new(FailingPostPlugin),
        ];

        let driver: PluginDriver = PluginDriver::new(plugins);

        let ctx: CompileContext<'_> =
            CompileContext::new("/repo", "a.ts", "console.log(1);");

        let args: PostArgs<'_> =
            PostArgs { file: "a.ts", code: "console.log(1);" };

        let err: anyhow::Error = driver.post(&ctx, &args).await.unwrap_err();

        assert!(format!("{err:#}").contains("`fail-post` post"), "{err:#}");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_pluginable_dispatch() {
        let plugin: Arc<dyn Pluginable> = Arc::new(OptionsPlugin);

        assert_eq!(plugin.call_name(), "options");

        let options: CompileOptions = CompileOptions {
            cwd: "/repo".to_string(),
            file: "a.ts".to_string(),
            code: "console.log(1);".to_string(),
        };

        let output: anyhow::Result<Option<PartialCompileOptions>> =
            plugin.call_options(&options).await;

        let output: PartialCompileOptions =
            output.unwrap().expect("options output");

        assert_eq!(output.cwd.as_deref(), Some("/changed"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_transform_skips_plugins_without_usage() {
        let allocator: Allocator = Allocator::default();

        let ctx: CompileContext<'_> =
            CompileContext::new("/repo", "a.ts", "console.log(1);");

        let parsed: ParseResult<'_> = parse(ParseOptions {
            context: &ctx,
            allocator: &allocator,
            file: "a.ts",
            code: "console.log(1);",
        })
        .unwrap();

        let program: Program<'_> = parsed.program;

        let args: TransformArgs<'_> = TransformArgs {
            allocator: &allocator,
            file: "a.ts",
            program: &program,
        };

        let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> =
            vec![Arc::new(PreOnlyPlugin::new(Arc::clone(&log)))];

        let driver: PluginDriver = PluginDriver::new(plugins);

        let replaced: Option<Program<'_>> =
            driver.transform(&ctx, &args).await.unwrap();

        let recorded: Vec<String> = log.lock().unwrap().clone();

        assert!(replaced.is_none());
        assert_eq!(driver.usage(), HookUsage::PRE);
        assert!(recorded.is_empty());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_driver_aggregates_usage() {
        let plugins: Vec<SharedPluginable> = vec![
            Arc::new(OptionsPlugin),
            Arc::new(FailingPrePlugin),
            Arc::new(MarkPlugin),
            Arc::new(FailingPostPlugin),
        ];

        let driver: PluginDriver = PluginDriver::new(plugins);

        assert_eq!(
            driver.usage(),
            HookUsage::OPTIONS
                | HookUsage::PRE
                | HookUsage::TRANSFORM
                | HookUsage::POST
        );
    }
}
