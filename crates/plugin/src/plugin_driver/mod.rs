pub mod hooks;

use std::sync::Arc;

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
    /// Plugins are partitioned by their declared [`HookUsage`];
    /// a hook only iterates the plugins that declared it.
    pub fn new(plugins: Vec<SharedPluginable>) -> Self {
        let mut usage: HookUsage = HookUsage::default();

        let mut options_plugins: Vec<SharedPluginable> = Vec::new();
        let mut pre_plugins: Vec<SharedPluginable> = Vec::new();
        let mut transform_plugins: Vec<SharedPluginable> = Vec::new();
        let mut post_plugins: Vec<SharedPluginable> = Vec::new();

        for plugin in &plugins {
            let declared: HookUsage = plugin.call_register_hook_usage();

            usage |= declared;

            if declared.contains(HookUsage::Options) {
                options_plugins.push(Arc::clone(plugin));
            }

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

    /// Run the `options` hook chain;
    /// each plugin mutates the carried [`CompileOptions`] in place.
    pub async fn options(
        &self,
        options: &mut CompileOptions,
    ) -> anyhow::Result<()> {
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

    /// Run the `transform` hook chain;
    /// each plugin mutates the carried [`oxc::ast::ast::Program`] in place.
    pub async fn transform<'a, 'ast>(
        &'a self,
        ctx: &'a CompileContext<'a>,
        args: &mut TransformArgs<'a, 'ast>,
    ) -> anyhow::Result<()> {
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
        parse,
    };

    use oxc::allocator::Allocator;
    use oxc::ast::ast::Program;

    use crate::_types::hooks::transform::{TransformArgs, TransformReturn};
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

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Options
        }

        async fn options(
            &self,
            options: &mut CompileOptions,
        ) -> anyhow::Result<()> {
            options.cwd = Some("/changed".into());
            Ok(())
        }
    }

    #[derive(Debug)]
    struct NoopOptionsPlugin;

    impl Plugin for NoopOptionsPlugin {
        fn name(&self) -> Cow<'static, str> {
            "noop-options".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Options
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

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Options
        }

        async fn options(
            &self,
            options: &mut CompileOptions,
        ) -> anyhow::Result<()> {
            self.observed.lock().unwrap().push(options.code.clone());

            Ok(())
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

        async fn options(
            &self,
            _options: &mut CompileOptions,
        ) -> anyhow::Result<()> {
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

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Pre | HookUsage::Post
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

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Pre
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

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Transform
        }

        fn transform<'a, 'ast>(
            &'a self,
            ctx: &'a CompileContext<'a>,
            args: TransformArgs<'a, 'ast>,
        ) -> impl Future<Output = TransformReturn> {
            async move {
                let code: &'ast str = args.allocator.alloc_str("\"mark\";");

                let file: &'ast str = args.allocator.alloc_str(args.file);

                let options: ParseOptions<'_, '_> = ParseOptions {
                    context: ctx,
                    allocator: args.allocator,
                    file,
                    code,
                };

                let parsed: ParseResult<'_> = parse(options).unwrap();

                *args.program = parsed.program;

                Ok(())
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

        fn transform<'a, 'ast>(
            &'a self,
            _ctx: &'a CompileContext<'a>,
            args: TransformArgs<'a, 'ast>,
        ) -> impl Future<Output = TransformReturn> {
            async move {
                self.record(args.program);
                Ok(())
            }
        }
    }

    #[derive(Debug)]
    struct RecordingMutatorPlugin {
        seen: Arc<Mutex<Vec<usize>>>,
    }

    impl RecordingMutatorPlugin {
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

    impl Plugin for RecordingMutatorPlugin {
        fn name(&self) -> Cow<'static, str> {
            "record-mutate".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Transform
        }

        fn transform<'a, 'ast>(
            &'a self,
            ctx: &'a CompileContext<'a>,
            args: TransformArgs<'a, 'ast>,
        ) -> impl Future<Output = TransformReturn> {
            async move {
                self.record(args.program);

                let code: &'ast str = args.allocator.alloc_str("\"mark\";");

                let file: &'ast str = args.allocator.alloc_str(args.file);

                let options: ParseOptions<'_, '_> = ParseOptions {
                    context: ctx,
                    allocator: args.allocator,
                    file,
                    code,
                };

                let parsed: ParseResult<'_> = parse(options).unwrap();

                *args.program = parsed.program;

                Ok(())
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

        fn transform<'a, 'ast>(
            &'a self,
            _ctx: &'a CompileContext<'a>,
            _args: TransformArgs<'a, 'ast>,
        ) -> impl Future<Output = TransformReturn> {
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

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Pre
        }

        async fn pre(
            &self,
            _ctx: &CompileContext<'_>,
            _args: &PreArgs<'_>,
        ) -> anyhow::Result<()> {
            self.record();
            Ok(())
        }

        fn transform<'a, 'ast>(
            &'a self,
            _ctx: &'a CompileContext<'a>,
            _args: TransformArgs<'a, 'ast>,
        ) -> impl Future<Output = TransformReturn> {
            async move {
                self.record();
                Ok(())
            }
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_options_chain_mutates_in_place() {
        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(OptionsPlugin),
            Plugin::new_shared(OptionsPlugin),
        ];

        let driver: PluginDriver = PluginDriver::new(plugins);

        let mut options: CompileOptions = CompileOptions {
            cwd: Some("/repo".into()),
            file: "a.ts".to_string(),
            code: "console.log(1);".to_string(),
        };

        driver.options(&mut options).await.unwrap();

        assert_eq!(options.cwd.as_deref(), Some("/changed"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_options_noop_keeps_current() {
        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(NoopOptionsPlugin)];

        let driver: PluginDriver = PluginDriver::new(plugins);

        let mut options: CompileOptions = CompileOptions {
            cwd: Some("/repo".into()),
            file: "a.ts".to_string(),
            code: "console.log(1);".to_string(),
        };

        driver.options(&mut options).await.unwrap();

        assert_eq!(options.cwd.as_deref(), Some("/repo"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_options_mutation_keeps_other_fields() {
        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(RewriteCodeOptionsPlugin)];

        let driver: PluginDriver = PluginDriver::new(plugins);

        let mut options: CompileOptions = CompileOptions {
            cwd: Some("/repo".into()),
            file: "a.ts".to_string(),
            code: "console.log(1);".to_string(),
        };

        driver.options(&mut options).await.unwrap();

        assert_eq!(options.cwd.as_deref(), Some("/repo"));
        assert_eq!(options.file, "a.ts");
        assert_eq!(options.code, "const rewritten = 7;");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_options_mutation_visible_to_later_plugin() {
        let observed: Arc<Mutex<Vec<String>>> =
            Arc::new(Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(RewriteCodeOptionsPlugin),
            Plugin::new_shared(ObserveOptionsPlugin::new(Arc::clone(
                &observed,
            ))),
        ];

        let driver: PluginDriver = PluginDriver::new(plugins);

        let mut options: CompileOptions = CompileOptions {
            file: "a.ts".to_string(),
            code: "console.log(1);".to_string(),
            ..Default::default()
        };

        driver.options(&mut options).await.unwrap();

        assert_eq!(options.code, "const rewritten = 7;");

        let recorded: Vec<String> = observed.lock().unwrap().clone();

        assert_eq!(recorded, vec!["const rewritten = 7;".to_string()],);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_options_error_propagates() {
        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(FailingOptionsPlugin)];

        let driver: PluginDriver = PluginDriver::new(plugins);

        let mut options: CompileOptions = CompileOptions {
            cwd: Some("/repo".into()),
            file: "a.ts".to_string(),
            code: "console.log(1);".to_string(),
        };

        let result: anyhow::Result<()> = driver.options(&mut options).await;

        assert!(result.is_err());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_options_error_names_plugin() {
        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(FailingOptionsPlugin)];

        let driver: PluginDriver = PluginDriver::new(plugins);

        let mut options: CompileOptions = CompileOptions {
            cwd: Some("/repo".into()),
            file: "a.ts".to_string(),
            code: "console.log(1);".to_string(),
        };

        let err: anyhow::Error =
            driver.options(&mut options).await.unwrap_err();

        assert!(
            format!("{err:#}").contains("`fail-options` options"),
            "{err:#}"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_pre_runs_all_plugins_in_order() {
        let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(OrderPlugin::new("first", Arc::clone(&log))),
            Plugin::new_shared(OrderPlugin::new("second", Arc::clone(&log))),
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
            Plugin::new_shared(OrderPlugin::new(
                "first",
                Arc::new(Mutex::new(Vec::new())),
            )),
            Plugin::new_shared(FailingPrePlugin),
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

        let mut program: Program<'_> = parsed.program;

        let mut args: TransformArgs<'_, '_> = TransformArgs {
            allocator: &allocator,
            file: "a.ts",
            program: &mut program,
        };

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(MarkPlugin),
            Plugin::new_shared(MarkPlugin),
        ];

        let driver: PluginDriver = PluginDriver::new(plugins);

        driver.transform(&ctx, &mut args).await.unwrap();

        // the chain swapped the root in place; the final program is the
        // last "mark" swap, still rooted in the allocator
        let out: String =
            oxc::codegen::Codegen::new().build(&*args.program).code;

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

        let mut program: Program<'_> = parsed.program;

        let mut args: TransformArgs<'_, '_> = TransformArgs {
            allocator: &allocator,
            file: "a.ts",
            program: &mut program,
        };

        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(FailingTransformPlugin)];

        let driver: PluginDriver = PluginDriver::new(plugins);

        let err: anyhow::Error =
            driver.transform(&ctx, &mut args).await.unwrap_err();

        assert!(
            format!("{err:#}").contains("`fail-transform` transform"),
            "{err:#}"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_transform_returns_original_program_by_reference() {
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

        let mut program: Program<'_> = parsed.program;

        let mut args: TransformArgs<'_, '_> = TransformArgs {
            allocator: &allocator,
            file: "a.ts",
            program: &mut program,
        };

        let seen: Arc<Mutex<Vec<usize>>> = Arc::new(Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(RecordingPlugin::new(Arc::clone(&seen)))];

        let driver: PluginDriver = PluginDriver::new(plugins);

        driver.transform(&ctx, &mut args).await.unwrap();

        let recorded: Vec<usize> = seen.lock().unwrap().clone();

        // no plugin mutated, so the hook runner must hand the original
        // program to the plugin by reference instead of cloning it into
        // the allocator first
        assert_eq!(recorded, vec![std::ptr::from_ref(&*args.program) as usize],);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_transform_mutations_flow_by_reference() {
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

        let mut program: Program<'_> = parsed.program;

        let mut args: TransformArgs<'_, '_> = TransformArgs {
            allocator: &allocator,
            file: "a.ts",
            program: &mut program,
        };

        let first_seen: Arc<Mutex<Vec<usize>>> =
            Arc::new(Mutex::new(Vec::new()));

        let second_seen: Arc<Mutex<Vec<usize>>> =
            Arc::new(Mutex::new(Vec::new()));

        let final_seen: Arc<Mutex<Vec<usize>>> =
            Arc::new(Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(RecordingMutatorPlugin::new(Arc::clone(
                &first_seen,
            ))),
            Plugin::new_shared(RecordingMutatorPlugin::new(Arc::clone(
                &second_seen,
            ))),
            Plugin::new_shared(RecordingPlugin::new(Arc::clone(&final_seen))),
        ];

        let driver: PluginDriver = PluginDriver::new(plugins);

        driver.transform(&ctx, &mut args).await.unwrap();

        let original: usize = std::ptr::from_ref(&*args.program) as usize;

        let first: usize = first_seen.lock().unwrap()[0];

        let second: usize = second_seen.lock().unwrap()[0];

        let final_seen: usize = final_seen.lock().unwrap()[0];

        // in-place mutation: every plugin observes the same root Program,
        // never a copy
        assert_eq!(first, original);

        assert_eq!(second, original);

        assert_eq!(final_seen, original);

        // the mutations are visible: the last root swap is the content
        let out: String =
            oxc::codegen::Codegen::new().build(&*args.program).code;

        assert!(out.contains("mark"), "{out}");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_post_runs_all_plugins_in_order() {
        let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(OrderPlugin::new("first", Arc::clone(&log))),
            Plugin::new_shared(OrderPlugin::new("second", Arc::clone(&log))),
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
            Plugin::new_shared(OrderPlugin::new(
                "first",
                Arc::new(Mutex::new(Vec::new())),
            )),
            Plugin::new_shared(FailingPostPlugin),
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
        let plugin: Arc<dyn Pluginable> = Plugin::new_shared(OptionsPlugin);

        assert_eq!(plugin.call_name(), "options");

        let mut options: CompileOptions = CompileOptions {
            file: "a.ts".to_string(),
            code: "console.log(1);".to_string(),
            ..Default::default()
        };

        plugin.call_options(&mut options).await.unwrap();

        assert_eq!(options.cwd.as_deref(), Some("/changed"));
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

        let mut program: Program<'_> = parsed.program;

        let mut args: TransformArgs<'_, '_> = TransformArgs {
            allocator: &allocator,
            file: "a.ts",
            program: &mut program,
        };

        let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(PreOnlyPlugin::new(Arc::clone(&log)))];

        let driver: PluginDriver = PluginDriver::new(plugins);

        driver.transform(&ctx, &mut args).await.unwrap();

        let recorded: Vec<String> = log.lock().unwrap().clone();

        assert_eq!(driver.usage(), HookUsage::Pre);
        assert!(recorded.is_empty());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_driver_aggregates_usage() {
        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(OptionsPlugin),
            Plugin::new_shared(FailingPrePlugin),
            Plugin::new_shared(MarkPlugin),
            Plugin::new_shared(FailingPostPlugin),
        ];

        let driver: PluginDriver = PluginDriver::new(plugins);

        assert_eq!(
            driver.usage(),
            HookUsage::Options
                | HookUsage::Pre
                | HookUsage::Transform
                | HookUsage::Post
        );
    }
}
