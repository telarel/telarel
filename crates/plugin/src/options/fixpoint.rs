use std::sync::Arc;

use anyhow::Context;

use crate::_types::context::CommonPluginContext;
use crate::_types::hooks::options::OptionsArgs;
use crate::plugin::pluginable::SharedPluginable;
use crate::plugin_driver::sort_plugins_by_hook_meta;

/// Maximum number of fixpoint iterations before aborting.
///
/// Constant and non-configurable: a pass that adds nothing terminates the
/// loop naturally; only plugin cycles reach this guard.
const MAX_OPTIONS_DEPTH: usize = 10;

/// Compares by content, not identity: a bridge may return fresh handles for
/// the same plugins every pass, so identity never matches while the work is
/// already done.
fn structurally_same(
    left: &[SharedPluginable],
    right: &[SharedPluginable],
) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(a, b)| a.call_name() == b.call_name())
}

/// Run the `options` hook as a fixpoint over the plugin list.
///
/// Plain sequential fold, NO driver: the processing list starts as
/// `args.plugins`; for each plugin, `Some` replaces the whole set and `None`
/// keeps it. Each pass ranks the current list by `options_meta`
/// (`[pre, normal, post]`, stable) and calls `options` on every listed plugin
/// not yet processed, in that order; later plugins observe earlier plugins'
/// changes (last-wins fold).
///
/// A returned list replaces the settled list WHOLESALE: same-length swaps,
/// reorders and truncations take effect, not just appended entries. A plugin
/// already run is never re-run (tracked by identity), so the loop ends once a
/// pass introduces nothing new. Exceeding [`MAX_OPTIONS_DEPTH`] aborts with
/// the offending plugin chain (the names that most recently GREW the list).
pub async fn options_fixpoint(
    ctx: &CommonPluginContext,
    args: &OptionsArgs,
) -> anyhow::Result<(OptionsArgs, Vec<SharedPluginable>)> {
    let mut args: OptionsArgs = args.clone();

    let mut list: Vec<SharedPluginable> = args.plugins.clone();

    // Held live: `Arc::ptr_eq` needs the handles alive once `list` is replaced.
    let mut processed: Vec<SharedPluginable> = Vec::new();

    let mut iterations: usize = 0;

    let mut chain: Vec<String> = Vec::new();

    loop {
        let pass_start: Vec<SharedPluginable> = list.clone();

        // Rank a COPY of the accumulator to drive this pass's iteration:
        // `pre`/`post` options meta affects EXECUTION order only. The
        // persisted `list` stays in append/registration order.
        let ranked: Vec<SharedPluginable> =
            sort_plugins_by_hook_meta(&list, |plugin| {
                plugin.call_options_meta()
            });

        let mut ran_any: bool = false;

        for plugin in &ranked {
            if processed.iter().any(|p| Arc::ptr_eq(p, plugin)) {
                continue;
            }

            processed.push(Arc::clone(plugin));

            ran_any = true;

            let before: usize = args.plugins.len();

            let next: Option<OptionsArgs> = plugin
                .call_options(ctx, &args)
                .await
                .with_context(|| format!("`{}` options", plugin.call_name()))?;

            if let Some(replaced) = next {
                args = replaced;
            }

            // Returned args replace the whole set; per-plugin chain
            // attribution: only a plugin whose returned args GREW `plugins`
            // (args that truncate below `before` add nothing) records
            // its name in the chain.
            if args.plugins.len() > before {
                chain.push(plugin.call_name().into_owned());
            }
        }

        list = args.plugins.clone();

        if !ran_any || structurally_same(&list, &pass_start) {
            break;
        }

        iterations += 1;

        // The guard fires when the TENTH consecutive pass that changed the
        // list completes: a converged fixpoint terminates before this; only
        // plugin cycles reach it.
        if iterations == MAX_OPTIONS_DEPTH {
            return Err(anyhow::anyhow!(
                "plugin depth exceeded (chain: {})",
                chain.join(" -> ")
            ));
        }
    }

    Ok((args, list))
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

    use telarel_common::HookUsage;

    use crate::_types::hooks::options::OptionsReturn;
    use crate::_types::order::{PluginHookMeta, PluginOrder};
    use crate::plugin::Plugin;

    use super::*;

    fn common_ctx() -> CommonPluginContext {
        CommonPluginContext::default()
    }

    fn make_args(plugins: Vec<SharedPluginable>) -> OptionsArgs {
        OptionsArgs {
            options: telarel_common::CompileOptions {
                file: "a.ts".to_string(),
                code: "console.log(1);".to_string(),
                ..telarel_common::CompileOptions::default()
            },
            plugins,
        }
    }

    #[derive(Debug)]
    struct OptionsPlugin {
        cwd: &'static str,
    }

    impl Plugin for OptionsPlugin {
        fn name(&self) -> Cow<'static, str> {
            "options".into()
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

                next.options.cwd = Some(self.cwd.to_string());

                Ok(Some(next))
            }
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

        fn options<'a>(
            &'a self,
            _ctx: &'a CommonPluginContext,
            args: &'a OptionsArgs,
        ) -> impl Future<Output = OptionsReturn> + Send {
            async move {
                let mut next: OptionsArgs = args.clone();

                next.options.code = "const rewritten = 7;".to_string();

                Ok(Some(next))
            }
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

        fn record(
            &self,
            code: &str,
        ) {
            let mut observed: MutexGuard<'_, Vec<String>> =
                self.observed.lock().unwrap();

            observed.push(code.to_string());
        }
    }

    impl Plugin for ObserveOptionsPlugin {
        fn name(&self) -> Cow<'static, str> {
            "observe-options".into()
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
                self.record(&args.options.code);

                Ok(None)
            }
        }
    }

    /// Appends its plugins to the args' plugin list on the first `times`
    /// invocations only, so the fixpoint can converge after them.
    #[derive(Debug)]
    struct AppendingOptionsPlugin {
        name: &'static str,
        append: Vec<SharedPluginable>,
        remaining: Mutex<usize>,
    }

    impl Plugin for AppendingOptionsPlugin {
        fn name(&self) -> Cow<'static, str> {
            self.name.into()
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
                let mut remaining: MutexGuard<'_, usize> =
                    self.remaining.lock().unwrap();

                if *remaining == 0 {
                    return Ok(None);
                }

                *remaining -= 1;

                let mut next: OptionsArgs = args.clone();

                next.plugins.extend(self.append.iter().cloned());

                Ok(Some(next))
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

    /// Returns args carrying ONLY `cwd`: every other field ("file") is
    /// dropped by the struct-level replace.
    #[derive(Debug)]
    struct ReplaceArgsDroppingFieldsOptionsPlugin;

    impl Plugin for ReplaceArgsDroppingFieldsOptionsPlugin {
        fn name(&self) -> Cow<'static, str> {
            "replace-args-dropping-fields".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Options
        }

        fn options<'a>(
            &'a self,
            _ctx: &'a CommonPluginContext,
            _args: &'a OptionsArgs,
        ) -> impl Future<Output = OptionsReturn> + Send {
            async {
                Ok(Some(OptionsArgs {
                    options: telarel_common::CompileOptions {
                        cwd: Some("/only".to_string()),
                        ..telarel_common::CompileOptions::default()
                    },
                    plugins: Vec::new(),
                }))
            }
        }
    }

    /// Appends ONE fresh instance of itself to the args' EXISTING plugins
    /// on every invocation: the plugin list grows by one per pass, so the
    /// fixpoint never converges and the depth guard must abort at pass
    /// [`MAX_OPTIONS_DEPTH`]. Growth-by-one (not doubling) keeps the
    /// guarded run fast while genuinely exercising ten adding passes.
    #[derive(Debug)]
    struct SelfAppendingPlugin {
        name: &'static str,
    }

    impl Plugin for SelfAppendingPlugin {
        fn name(&self) -> Cow<'static, str> {
            self.name.into()
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

                next.plugins.push(Plugin::new_shared(SelfAppendingPlugin {
                    name: self.name,
                }));

                Ok(Some(next))
            }
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_options_fold_runs_in_list_order_last_wins() {
        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(OptionsPlugin { cwd: "/first" }),
            Plugin::new_shared(OptionsPlugin { cwd: "/second" }),
        ];

        let args: OptionsArgs = make_args(plugins);

        let (settled, list): (OptionsArgs, Vec<SharedPluginable>) =
            options_fixpoint(&common_ctx(), &args).await.unwrap();

        assert_eq!(settled.options.cwd.as_deref(), Some("/second"));

        assert_eq!(list.len(), 2);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_options_none_keeps_current_args() {
        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(OptionsPlugin { cwd: "/first" }),
            Plugin::new_shared(NoopOptionsPlugin),
        ];

        let args: OptionsArgs = make_args(plugins);

        let (settled, _list): (OptionsArgs, Vec<SharedPluginable>) =
            options_fixpoint(&common_ctx(), &args).await.unwrap();

        assert_eq!(settled.options.cwd.as_deref(), Some("/first"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_options_replacement_visible_to_later_plugin() {
        let observed: Arc<Mutex<Vec<String>>> =
            Arc::new(Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(RewriteCodeOptionsPlugin),
            Plugin::new_shared(ObserveOptionsPlugin::new(Arc::clone(
                &observed,
            ))),
        ];

        let args: OptionsArgs = make_args(plugins);

        let (settled, _list): (OptionsArgs, Vec<SharedPluginable>) =
            options_fixpoint(&common_ctx(), &args).await.unwrap();

        assert_eq!(settled.options.code, "const rewritten = 7;");

        let recorded: Vec<String> = observed.lock().unwrap().clone();

        assert_eq!(recorded, vec!["const rewritten = 7;".to_string()]);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_options_error_propagates_with_plugin_name() {
        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(NoopOptionsPlugin),
            Plugin::new_shared(FailingOptionsPlugin),
        ];

        let args: OptionsArgs = make_args(plugins);

        let err: anyhow::Error =
            options_fixpoint(&common_ctx(), &args).await.unwrap_err();

        assert!(
            format!("{err:#}").contains("`fail-options` options"),
            "{err:#}"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_added_plugin_gets_options_hook_run() {
        let observed: Arc<Mutex<Vec<String>>> =
            Arc::new(Mutex::new(Vec::new()));

        let injected: SharedPluginable = Plugin::new_shared(
            ObserveOptionsPlugin::new(Arc::clone(&observed)),
        );

        // The root plugin appends the injected plugin once; the injected
        // plugin must get its own `options` hook run in the next pass.
        let root: SharedPluginable =
            Plugin::new_shared(AppendingOptionsPlugin {
                name: "append-observe",
                append: vec![injected],
                remaining: Mutex::new(1),
            });

        let plugins: Vec<SharedPluginable> = vec![root];

        let args: OptionsArgs = make_args(plugins);

        let (settled, list): (OptionsArgs, Vec<SharedPluginable>) =
            options_fixpoint(&common_ctx(), &args).await.unwrap();

        assert_eq!(list.len(), 2);

        assert_eq!(settled.plugins.len(), 2);

        let recorded: Vec<String> = observed.lock().unwrap().clone();

        assert_eq!(recorded, vec!["console.log(1);".to_string()]);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_added_plugin_late_in_list_keeps_fold_order() {
        // A plugin that appends a rewrite plugin: the appended plugin's
        // `options` hook must run in the pass after it joins the list, so
        // its rewrite is visible in the settled args.
        let appended: SharedPluginable =
            Plugin::new_shared(RewriteCodeOptionsPlugin);

        let root: SharedPluginable =
            Plugin::new_shared(AppendingOptionsPlugin {
                name: "append-rewrite",
                append: vec![appended],
                remaining: Mutex::new(1),
            });

        let plugins: Vec<SharedPluginable> = vec![root];

        let args: OptionsArgs = make_args(plugins);

        let (settled, list): (OptionsArgs, Vec<SharedPluginable>) =
            options_fixpoint(&common_ctx(), &args).await.unwrap();

        assert_eq!(settled.options.code, "const rewritten = 7;");

        let names: Vec<String> =
            list.iter().map(|p| p.call_name().into_owned()).collect();

        assert_eq!(
            names,
            vec![
                "append-rewrite".to_string(),
                "rewrite-code-options".to_string()
            ]
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_cycle_guarded_by_depth_error_naming_chain() {
        // A plugin that appends a fresh copy of itself on every pass: the
        // fixpoint never converges, so the depth guard must abort at the
        // TENTH consecutive pass (not the eleventh) with the offending
        // chain named in the message. The runner runs on a helper thread
        // behind a 30 recv_timeout: a guard firing even one pass late
        // bloats the run super-linearly (the self-referential args clone
        // doubles per pass), so an over-deep run FAILS via the timeout
        // instead of creeping past a minute.
        let (tx, rx): (
            std::sync::mpsc::Sender<anyhow::Error>,
            std::sync::mpsc::Receiver<anyhow::Error>,
        ) = std::sync::mpsc::channel::<anyhow::Error>();

        std::thread::spawn(move || {
            let runtime: tokio::runtime::Runtime =
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("test runtime builds");

            let plugins: Vec<SharedPluginable> =
                vec![Plugin::new_shared(SelfAppendingPlugin {
                    name: "cycle-a",
                })];

            let args: OptionsArgs = make_args(plugins);

            let outcome: anyhow::Result<(OptionsArgs, Vec<SharedPluginable>)> =
                runtime.block_on(async {
                    options_fixpoint(&common_ctx(), &args).await
                });

            if let Some(error) = outcome.err() {
                let _ = tx.send(error);
            }
        });

        let err: anyhow::Error = rx
            .recv_timeout(std::time::Duration::from_secs(30))
            .expect("depth guard must abort a self-appending cycle");

        let message: String = format!("{err:#}");

        assert!(
            message.contains("plugin depth exceeded (chain: "),
            "{message}"
        );

        assert!(message.contains("cycle-a"), "{message}");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_duplicates_allowed_order_stable() {
        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(OptionsPlugin { cwd: "/first" }),
            Plugin::new_shared(OptionsPlugin { cwd: "/first" }),
            Plugin::new_shared(OptionsPlugin { cwd: "/second" }),
        ];

        let args: OptionsArgs = make_args(plugins.clone());

        let (settled, list): (OptionsArgs, Vec<SharedPluginable>) =
            options_fixpoint(&common_ctx(), &args).await.unwrap();

        // No dedup: the list keeps every entry in final array order.
        assert_eq!(list.len(), 3);

        assert!(Arc::ptr_eq(&list[0], &plugins[0]));

        assert!(Arc::ptr_eq(&list[1], &plugins[1]));

        assert!(Arc::ptr_eq(&list[2], &plugins[2]));

        assert_eq!(settled.plugins.len(), 3);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_options_some_replaces_whole_args_drops_fields() {
        // Struct-level replace: returned args REPLACE the whole set, so
        // every field they do not carry falls back to `CompileOptions`'s
        // own defaults (the input args' `file: "a.ts"` is NOT preserved).
        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(ReplaceArgsDroppingFieldsOptionsPlugin)];

        let args: OptionsArgs = make_args(plugins);

        let (settled, _list): (OptionsArgs, Vec<SharedPluginable>) =
            options_fixpoint(&common_ctx(), &args).await.unwrap();

        assert_eq!(settled.options.cwd.as_deref(), Some("/only"));

        assert_eq!(settled.options.file, "index.js");

        assert_eq!(settled.options.code, "");
    }

    #[derive(Debug)]
    struct MetaRecordingOptionsPlugin {
        name: &'static str,
        order: Option<PluginOrder>,
        log: Arc<Mutex<Vec<String>>>,
    }

    impl MetaRecordingOptionsPlugin {
        fn new(
            name: &'static str,
            order: Option<PluginOrder>,
            log: &Arc<Mutex<Vec<String>>>,
        ) -> Self {
            Self { name, order, log: Arc::clone(log) }
        }
    }

    impl Plugin for MetaRecordingOptionsPlugin {
        fn name(&self) -> Cow<'static, str> {
            self.name.into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Options
        }

        fn options_meta(&self) -> Option<PluginHookMeta> {
            Some(PluginHookMeta { order: self.order })
        }

        fn options<'a>(
            &'a self,
            _ctx: &'a CommonPluginContext,
            _args: &'a OptionsArgs,
        ) -> impl Future<Output = OptionsReturn> + Send {
            async move {
                let mut log: MutexGuard<'_, Vec<String>> =
                    self.log.lock().unwrap();

                log.push(self.name.to_string());

                Ok(None)
            }
        }
    }

    /// Records its own name, and on the first `times` invocations appends
    /// `append` to the args' plugin list so the fixpoint runs another pass.
    #[derive(Debug)]
    struct MetaAppendingOptionsPlugin {
        name: &'static str,
        order: Option<PluginOrder>,
        append: Vec<SharedPluginable>,
        remaining: Mutex<usize>,
        log: Arc<Mutex<Vec<String>>>,
    }

    impl Plugin for MetaAppendingOptionsPlugin {
        fn name(&self) -> Cow<'static, str> {
            self.name.into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Options
        }

        fn options_meta(&self) -> Option<PluginHookMeta> {
            Some(PluginHookMeta { order: self.order })
        }

        fn options<'a>(
            &'a self,
            _ctx: &'a CommonPluginContext,
            args: &'a OptionsArgs,
        ) -> impl Future<Output = OptionsReturn> + Send {
            async move {
                let mut log: MutexGuard<'_, Vec<String>> =
                    self.log.lock().unwrap();

                log.push(self.name.to_string());

                drop(log);

                let mut remaining: MutexGuard<'_, usize> =
                    self.remaining.lock().unwrap();

                if *remaining == 0 {
                    return Ok(None);
                }

                *remaining -= 1;

                let mut next: OptionsArgs = args.clone();

                next.plugins.extend(self.append.iter().cloned());

                Ok(Some(next))
            }
        }
    }

    /// Records its own name and replaces the WHOLE plugin list with a
    /// preconfigured list on every call.
    #[derive(Debug)]
    struct ReturnsListOptionsPlugin {
        name: &'static str,
        list: Arc<Mutex<Vec<SharedPluginable>>>,
        log: Arc<Mutex<Vec<String>>>,
    }

    impl ReturnsListOptionsPlugin {
        fn new(
            name: &'static str,
            list: &Arc<Mutex<Vec<SharedPluginable>>>,
            log: &Arc<Mutex<Vec<String>>>,
        ) -> Self {
            Self { name, list: Arc::clone(list), log: Arc::clone(log) }
        }
    }

    impl Plugin for ReturnsListOptionsPlugin {
        fn name(&self) -> Cow<'static, str> {
            self.name.into()
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
                let mut log: MutexGuard<'_, Vec<String>> =
                    self.log.lock().unwrap();

                log.push(self.name.to_string());

                drop(log);

                let mut next: OptionsArgs = args.clone();

                next.plugins = self.list.lock().unwrap().clone();

                Ok(Some(next))
            }
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_options_pre_meta_ranks_before_normal() {
        let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(MetaRecordingOptionsPlugin::new(
                "normal", None, &log,
            )),
            Plugin::new_shared(MetaRecordingOptionsPlugin::new(
                "pre",
                Some(PluginOrder::Pre),
                &log,
            )),
        ];

        let args: OptionsArgs = make_args(plugins);

        let (_settled, _list): (OptionsArgs, Vec<SharedPluginable>) =
            options_fixpoint(&common_ctx(), &args).await.unwrap();

        assert_eq!(
            log.lock().unwrap().clone(),
            vec!["pre".to_string(), "normal".to_string()],
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_injected_meta_plugin_ranked_after_joining_list() {
        let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let injected: SharedPluginable =
            Plugin::new_shared(MetaRecordingOptionsPlugin::new(
                "injected-pre",
                Some(PluginOrder::Pre),
                &log,
            ));

        let root: SharedPluginable =
            Plugin::new_shared(MetaAppendingOptionsPlugin {
                name: "root",
                order: None,
                append: vec![injected],
                remaining: Mutex::new(1),
                log: Arc::clone(&log),
            });

        let plugins: Vec<SharedPluginable> = vec![root];

        let args: OptionsArgs = make_args(plugins);

        let (_settled, list): (OptionsArgs, Vec<SharedPluginable>) =
            options_fixpoint(&common_ctx(), &args).await.unwrap();

        // pass 1 runs `root` alone; pass 2 re-ranks the joined list so the
        // injected `Pre` plugin runs before the normal `root`. `root` was
        // already processed by identity, so it is NOT re-run.
        assert_eq!(
            log.lock().unwrap().clone(),
            vec!["root".to_string(), "injected-pre".to_string()],
        );

        // The RETURNED list stays in append/registration order: the injected
        // plugin was appended after `root`, so it follows it here even though
        // it executed first in pass 2.
        let names: Vec<String> =
            list.iter().map(|plugin| plugin.call_name().into_owned()).collect();

        assert_eq!(names, vec!["root".to_string(), "injected-pre".to_string()]);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_options_ranked_execution_but_registration_returned_list() {
        let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(MetaRecordingOptionsPlugin::new(
                "a", None, &log,
            )),
            Plugin::new_shared(MetaRecordingOptionsPlugin::new(
                "pre",
                Some(PluginOrder::Pre),
                &log,
            )),
            Plugin::new_shared(MetaRecordingOptionsPlugin::new(
                "b", None, &log,
            )),
        ];

        let args: OptionsArgs = make_args(plugins);

        let (_settled, list): (OptionsArgs, Vec<SharedPluginable>) =
            options_fixpoint(&common_ctx(), &args).await.unwrap();

        assert_eq!(
            log.lock().unwrap().clone(),
            vec!["pre".to_string(), "a".to_string(), "b".to_string(),],
        );

        let names: Vec<String> =
            list.iter().map(|plugin| plugin.call_name().into_owned()).collect();

        assert_eq!(
            names,
            vec!["a".to_string(), "pre".to_string(), "b".to_string()],
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_same_length_swap_takes_effect_and_runs_new_plugin() {
        let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let probe_b: SharedPluginable = Plugin::new_shared(
            MetaRecordingOptionsPlugin::new("probe-b", None, &log),
        );

        let replacement: Arc<Mutex<Vec<SharedPluginable>>> =
            Arc::new(Mutex::new(vec![probe_b]));

        let replacer: SharedPluginable = Plugin::new_shared(
            ReturnsListOptionsPlugin::new("replacer", &replacement, &log),
        );

        replacement.lock().unwrap().insert(0, Arc::clone(&replacer));

        let probe_a: SharedPluginable = Plugin::new_shared(
            MetaRecordingOptionsPlugin::new("probe-a", None, &log),
        );

        let plugins: Vec<SharedPluginable> = vec![replacer, probe_a];

        let args: OptionsArgs = make_args(plugins);

        let (_settled, list): (OptionsArgs, Vec<SharedPluginable>) =
            options_fixpoint(&common_ctx(), &args).await.unwrap();

        // A same-length returned list (swap `probe-a` for `probe-b`) is
        // adopted wholesale: the settled list drops `probe-a`.
        let names: Vec<String> =
            list.iter().map(|plugin| plugin.call_name().into_owned()).collect();

        assert_eq!(names, vec!["replacer".to_string(), "probe-b".to_string()],);

        // The newly appeared `probe-b` gets its options hook run next pass.
        assert_eq!(
            log.lock().unwrap().clone(),
            vec![
                "replacer".to_string(),
                "probe-a".to_string(),
                "probe-b".to_string(),
            ],
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_prefix_reorder_converges_without_reruns() {
        let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let b: SharedPluginable = Plugin::new_shared(
            MetaRecordingOptionsPlugin::new("b", None, &log),
        );

        let replacement: Arc<Mutex<Vec<SharedPluginable>>> =
            Arc::new(Mutex::new(vec![Arc::clone(&b)]));

        let a: SharedPluginable = Plugin::new_shared(
            ReturnsListOptionsPlugin::new("a", &replacement, &log),
        );

        replacement.lock().unwrap().push(Arc::clone(&a));

        let plugins: Vec<SharedPluginable> = vec![a, b];

        let args: OptionsArgs = make_args(plugins);

        let (_settled, list): (OptionsArgs, Vec<SharedPluginable>) =
            options_fixpoint(&common_ctx(), &args).await.unwrap();

        let names: Vec<String> =
            list.iter().map(|plugin| plugin.call_name().into_owned()).collect();

        assert_eq!(names, vec!["b".to_string(), "a".to_string()]);

        // Both identities were already processed in pass 1; the reorder
        // introduces no unprocessed entry, so neither is re-run.
        assert_eq!(
            log.lock().unwrap().clone(),
            vec!["a".to_string(), "b".to_string()],
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_truncation_drops_plugins_after_they_ran() {
        let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let probe_a: SharedPluginable = Plugin::new_shared(
            MetaRecordingOptionsPlugin::new("probe-a", None, &log),
        );

        let probe_b: SharedPluginable = Plugin::new_shared(
            MetaRecordingOptionsPlugin::new("probe-b", None, &log),
        );

        let replacement: Arc<Mutex<Vec<SharedPluginable>>> =
            Arc::new(Mutex::new(Vec::new()));

        let trunc: SharedPluginable = Plugin::new_shared(
            ReturnsListOptionsPlugin::new("trunc", &replacement, &log),
        );

        replacement.lock().unwrap().push(Arc::clone(&trunc));

        let plugins: Vec<SharedPluginable> = vec![trunc, probe_a, probe_b];

        let args: OptionsArgs = make_args(plugins);

        let (_settled, list): (OptionsArgs, Vec<SharedPluginable>) =
            options_fixpoint(&common_ctx(), &args).await.unwrap();

        let names: Vec<String> =
            list.iter().map(|plugin| plugin.call_name().into_owned()).collect();

        assert_eq!(names, vec!["trunc".to_string()]);

        // The dropped plugins were processed before the truncating return,
        // so no error; the settled list simply excludes them.
        assert_eq!(
            log.lock().unwrap().clone(),
            vec![
                "trunc".to_string(),
                "probe-a".to_string(),
                "probe-b".to_string(),
            ],
        );
    }
}
