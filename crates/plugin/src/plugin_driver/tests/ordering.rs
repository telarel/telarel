use super::*;

#[tokio::test(flavor = "current_thread")]
async fn test_driver_aggregates_usage() {
    // An options-only plugin still contributes its declared bit to the
    // aggregate. No options partition exists because the fixpoint runner
    // owns the options stage.
    let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    let plugins: Vec<SharedPluginable> = vec![
        Plugin::new_shared(ObserverPlugin {
            name: "prepare-only",
            usage: HookUsage::Prepare,
            fail: false,
            log: Arc::clone(&log),
        }),
        Plugin::new_shared(ObserverPlugin {
            name: "finalize-only",
            usage: HookUsage::Finalize,
            fail: false,
            log: Arc::clone(&log),
        }),
        Plugin::new_shared(MarkPlugin),
    ];

    let driver: PluginDriver = PluginDriver::new(plugins);

    assert_eq!(
        driver.usage(),
        HookUsage::Prepare | HookUsage::Transform | HookUsage::Finalize
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_start_runs_all_plugins_not_partitioned() {
    // `start` reaches ALL settled plugins, including those that only
    // declared Prepare (R5: invoke on all plugins).
    let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    let plugins: Vec<SharedPluginable> = vec![
        Plugin::new_shared(NotifyOrderPlugin {
            name: "first",
            usage: HookUsage::Prepare,
            log: Arc::clone(&log),
        }),
        Plugin::new_shared(NotifyOrderPlugin {
            name: "second",
            usage: HookUsage::Prepare,
            log: Arc::clone(&log),
        }),
    ];

    let driver: PluginDriver = PluginDriver::new(plugins);

    let ctx: PluginContext<'_> = make_plugin_ctx();

    driver.start(&ctx, &make_start_args()).await.unwrap();

    assert_eq!(
        *log.lock().unwrap(),
        vec![String::from("first:start"), String::from("second:start")],
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_start_aborts_on_first_error() {
    let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    let plugins: Vec<SharedPluginable> = vec![
        Plugin::new_shared(StartPlugin {
            name: "fail-start",
            fail: true,
            log: Arc::clone(&log),
        }),
        Plugin::new_shared(StartPlugin {
            name: "never",
            fail: false,
            log: Arc::clone(&log),
        }),
    ];

    let driver: PluginDriver = PluginDriver::new(plugins);

    let ctx: PluginContext<'_> = make_plugin_ctx();

    let err: PluginError =
        driver.start(&ctx, &make_start_args()).await.unwrap_err();

    assert_eq!(err.message(), "fail-start");

    assert_eq!(*log.lock().unwrap(), vec![String::from("fail-start")]);
}

#[tokio::test(flavor = "current_thread")]
async fn test_start_receives_resolved_options() {
    let started: Arc<Mutex<Option<PathBuf>>> = Arc::new(Mutex::new(None));

    let ended: Arc<Mutex<Vec<Option<String>>>> =
        Arc::new(Mutex::new(Vec::new()));

    let plugins: Vec<SharedPluginable> =
        vec![Plugin::new_shared(NotifyArgsPlugin {
            started: Arc::clone(&started),
            ended: Arc::clone(&ended),
        })];

    let driver: PluginDriver = PluginDriver::new(plugins);

    let ctx: PluginContext<'_> = make_plugin_ctx();

    driver.start(&ctx, &make_start_args()).await.unwrap();

    assert_eq!(started.lock().unwrap().as_deref(), Some(Path::new("/repo")));
}

#[tokio::test(flavor = "current_thread")]
async fn test_end_receives_error_only() {
    let started: Arc<Mutex<Option<PathBuf>>> = Arc::new(Mutex::new(None));

    let ended: Arc<Mutex<Vec<Option<String>>>> =
        Arc::new(Mutex::new(Vec::new()));

    let plugins: Vec<SharedPluginable> =
        vec![Plugin::new_shared(NotifyArgsPlugin {
            started: Arc::clone(&started),
            ended: Arc::clone(&ended),
        })];

    let driver: PluginDriver = PluginDriver::new(plugins);

    let ctx: PluginContext<'_> = make_plugin_ctx();

    driver.end(&ctx, &make_end_args()).await.unwrap();

    assert_eq!(*ended.lock().unwrap(), vec![None]);

    driver
        .end(
            &ctx,
            &crate::EndArgs {
                error: Some(telarel_common::CompileError::new("boom")),
            },
        )
        .await
        .unwrap();

    assert_eq!(*ended.lock().unwrap(), vec![None, Some(String::from("boom"))]);
}

#[tokio::test(flavor = "current_thread")]
async fn test_end_runs_all_plugins_after_first_error() {
    let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    let plugins: Vec<SharedPluginable> = vec![
        Plugin::new_shared(EndPlugin {
            name: "first",
            fail: true,
            log: Arc::clone(&log),
        }),
        Plugin::new_shared(EndPlugin {
            name: "second",
            fail: false,
            log: Arc::clone(&log),
        }),
        Plugin::new_shared(EndPlugin {
            name: "third",
            fail: false,
            log: Arc::clone(&log),
        }),
    ];

    let driver: PluginDriver = PluginDriver::new(plugins);

    let ctx: PluginContext<'_> = make_plugin_ctx();

    let err: PluginError =
        driver.end(&ctx, &make_end_args()).await.unwrap_err();

    assert_eq!(err.message(), "first");

    assert_eq!(
        *log.lock().unwrap(),
        vec![
            String::from("first"),
            String::from("second"),
            String::from("third"),
        ],
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_end_first_error_reported() {
    let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    let plugins: Vec<SharedPluginable> = vec![
        Plugin::new_shared(EndPlugin {
            name: "fail-end-1",
            fail: true,
            log: Arc::clone(&log),
        }),
        Plugin::new_shared(EndPlugin {
            name: "fail-end-2",
            fail: true,
            log: Arc::clone(&log),
        }),
    ];

    let driver: PluginDriver = PluginDriver::new(plugins);

    let ctx: PluginContext<'_> = make_plugin_ctx();

    let err: PluginError =
        driver.end(&ctx, &make_end_args()).await.unwrap_err();

    assert_eq!(err.message(), "fail-end-1");

    assert_eq!(
        *log.lock().unwrap(),
        vec![String::from("fail-end-1"), String::from("fail-end-2")],
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_settled_names_preserves_duplicates() {
    let plugins: Vec<SharedPluginable> =
        vec![Plugin::new_shared(MarkPlugin), Plugin::new_shared(MarkPlugin)];

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
    start_meta: Option<PluginHookMeta>,
    end_meta: Option<PluginHookMeta>,
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
            start_meta: None,
            end_meta: None,
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

    fn with_start_meta(
        mut self,
        order: PluginOrder,
    ) -> Self {
        self.start_meta = Some(PluginHookMeta { order: Some(order) });

        self
    }

    fn with_end_meta(
        mut self,
        order: PluginOrder,
    ) -> Self {
        self.end_meta = Some(PluginHookMeta { order: Some(order) });

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
        HookUsage::Prepare
            | HookUsage::Transform
            | HookUsage::Finalize
            | HookUsage::Start
            | HookUsage::End
    }

    fn start<'a>(
        &'a self,
        _ctx: &'a PluginContext<'_>,
        _args: &'a crate::StartArgs,
    ) -> impl Future<Output = NotifyReturn> + Send {
        async move {
            self.record("start");

            Ok(())
        }
    }

    fn start_meta(&self) -> Option<PluginHookMeta> {
        self.start_meta
    }

    fn prepare<'a>(
        &'a self,
        _ctx: &'a PluginContext<'_>,
        _args: PrepareArgs<'a>,
    ) -> impl Future<Output = NotifyReturn> + Send + 'a {
        async move {
            self.record("prepare");

            Ok(())
        }
    }

    fn prepare_meta(&self) -> Option<PluginHookMeta> {
        self.prepare_meta
    }

    fn transform<'a>(
        &'a self,
        _ctx: &'a PluginContext<'_>,
        _args: TransformArgs,
    ) -> impl Future<Output = TransformReturn> + Send + 'a {
        async move {
            self.record("transform");

            Ok(None)
        }
    }

    fn transform_meta(&self) -> Option<PluginHookMeta> {
        self.transform_meta
    }

    fn finalize<'a>(
        &'a self,
        _ctx: &'a PluginContext<'_>,
        _args: FinalizeArgs<'a>,
    ) -> impl Future<Output = NotifyReturn> + Send + 'a {
        async move {
            self.record("finalize");

            Ok(())
        }
    }

    fn finalize_meta(&self) -> Option<PluginHookMeta> {
        self.finalize_meta
    }

    fn end<'a>(
        &'a self,
        _ctx: &'a PluginContext<'_>,
        _args: &'a crate::EndArgs,
    ) -> impl Future<Output = NotifyReturn> + Send {
        async move {
            self.record("end");

            Ok(())
        }
    }

    fn end_meta(&self) -> Option<PluginHookMeta> {
        self.end_meta
    }
}

#[tokio::test(flavor = "current_thread")]
async fn test_prepare_pre_normal_post_interleaving() {
    let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    let plugins: Vec<SharedPluginable> = vec![
        Plugin::new_shared(MetaOrderPlugin::new("a", &log)),
        Plugin::new_shared(
            MetaOrderPlugin::new("b", &log).with_prepare_meta(PluginOrder::Pre),
        ),
        Plugin::new_shared(
            MetaOrderPlugin::new("c", &log)
                .with_prepare_meta(PluginOrder::Post),
        ),
    ];

    let driver: PluginDriver = PluginDriver::new(plugins);

    let ctx: PluginContext<'_> = make_plugin_ctx();

    let ast: Ast = test_ast();

    driver.prepare(&ctx, PrepareArgs { ast: &ast }).await.unwrap();

    assert_eq!(
        *log.lock().unwrap(),
        vec![
            String::from("b:prepare"),
            String::from("a:prepare"),
            String::from("c:prepare"),
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

    let ctx: PluginContext<'_> = make_plugin_ctx();

    let ast: Ast = test_ast();

    driver.prepare(&ctx, PrepareArgs { ast: &ast }).await.unwrap();

    assert_eq!(
        *log.lock().unwrap(),
        vec![
            String::from("pre-1:prepare"),
            String::from("pre-2:prepare"),
            String::from("normal:prepare"),
            String::from("post-1:prepare"),
            String::from("post-2:prepare"),
        ],
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_meta_is_per_hook() {
    // `meta` is `Pre` on prepare but normal on finalize: it leads the
    // prepare pass yet trails `plain` for finalize.
    let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    let plugins: Vec<SharedPluginable> = vec![
        Plugin::new_shared(MetaOrderPlugin::new("plain", &log)),
        Plugin::new_shared(
            MetaOrderPlugin::new("meta", &log)
                .with_prepare_meta(PluginOrder::Pre),
        ),
    ];

    let driver: PluginDriver = PluginDriver::new(plugins);

    let ctx: PluginContext<'_> = make_plugin_ctx();

    let ast: Ast = test_ast();

    driver.prepare(&ctx, PrepareArgs { ast: &ast }).await.unwrap();

    assert_eq!(
        log.lock().unwrap().clone(),
        vec![String::from("meta:prepare"), String::from("plain:prepare")],
    );

    log.lock().unwrap().clear();

    driver.finalize(&ctx, FinalizeArgs { ast: &ast }).await.unwrap();

    assert_eq!(
        log.lock().unwrap().clone(),
        vec![String::from("plain:finalize"), String::from("meta:finalize")],
    );
}

#[test]
fn test_sort_plugins_by_hook_meta_buckets_and_stability() {
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

    let names: Vec<String> =
        ranked.iter().map(|plugin| plugin.call_name().into_owned()).collect();

    assert_eq!(
        names,
        vec![
            String::from("pre1"),
            String::from("pre2"),
            String::from("n1"),
            String::from("n2"),
            String::from("post1"),
            String::from("post2"),
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

    let names: Vec<String> =
        ranked.iter().map(|plugin| plugin.call_name().into_owned()).collect();

    assert_eq!(names, vec![String::from("a"), String::from("b")]);
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
            MetaOrderPlugin::new("meta-none", &log).with_prepare_meta_none(),
        ),
        Plugin::new_shared(MetaOrderPlugin::new("plain", &log)),
    ];

    let ranked: Vec<SharedPluginable> =
        sort_plugins_by_hook_meta(&plugins, |plugin| {
            plugin.call_prepare_meta()
        });

    let names: Vec<String> =
        ranked.iter().map(|plugin| plugin.call_name().into_owned()).collect();

    assert_eq!(
        names,
        vec![
            String::from("pre"),
            String::from("meta-none"),
            String::from("plain"),
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

    let ctx: PluginContext<'_> = make_plugin_ctx();

    let ast: Ast = test_ast();

    let args: TransformArgs = TransformArgs { ast: ast.clone() };

    driver.transform(&ctx, args).await.unwrap();

    assert_eq!(
        log.lock().unwrap().clone(),
        vec![
            String::from("b:transform"),
            String::from("a:transform"),
            String::from("c:transform"),
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

    let ctx: PluginContext<'_> = make_plugin_ctx();

    let ast: Ast = test_ast();

    driver.finalize(&ctx, FinalizeArgs { ast: &ast }).await.unwrap();

    assert_eq!(
        log.lock().unwrap().clone(),
        vec![
            String::from("b:finalize"),
            String::from("a:finalize"),
            String::from("c:finalize"),
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
        HookUsage::Start
    }

    fn options_meta(&self) -> Option<PluginHookMeta> {
        self.options_meta
    }

    fn start<'a>(
        &'a self,
        _ctx: &'a PluginContext<'_>,
        _args: &'a crate::StartArgs,
    ) -> impl Future<Output = NotifyReturn> + Send {
        async move {
            self.log.lock().unwrap().push(self.name.to_string());

            Ok(())
        }
    }
}

#[tokio::test(flavor = "current_thread")]
async fn test_options_meta_does_not_leak_into_start_order() {
    // A plugin with `options: Pre` must NOT be promoted in the start
    // bucket: options meta is per-hook and the driver only reads
    // start meta. `pre-options` still trails the normal plugin in
    // registration order.
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

    let ctx: PluginContext<'_> = make_plugin_ctx();

    driver.start(&ctx, &make_start_args()).await.unwrap();

    assert_eq!(
        *log.lock().unwrap(),
        vec![String::from("normal"), String::from("pre-options")],
    );

    assert_eq!(
        driver.settled_names(),
        vec![String::from("normal"), String::from("pre-options")],
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_notify_hooks_ordered_while_settled_names_stay_registration() {
    let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    let plugins: Vec<SharedPluginable> = vec![
        Plugin::new_shared(MetaOrderPlugin::new("a", &log)),
        Plugin::new_shared(
            MetaOrderPlugin::new("b", &log)
                .with_start_meta(PluginOrder::Pre)
                .with_end_meta(PluginOrder::Pre),
        ),
        Plugin::new_shared(
            MetaOrderPlugin::new("c", &log)
                .with_start_meta(PluginOrder::Post)
                .with_end_meta(PluginOrder::Post),
        ),
    ];

    let driver: PluginDriver = PluginDriver::new(plugins);

    let ctx: PluginContext<'_> = make_plugin_ctx();

    driver.start(&ctx, &make_start_args()).await.unwrap();

    assert_eq!(
        log.lock().unwrap().clone(),
        vec![
            String::from("b:start"),
            String::from("a:start"),
            String::from("c:start"),
        ],
    );

    log.lock().unwrap().clear();

    driver.end(&ctx, &make_end_args()).await.unwrap();

    assert_eq!(
        log.lock().unwrap().clone(),
        vec![
            String::from("b:end"),
            String::from("a:end"),
            String::from("c:end"),
        ],
    );

    assert_eq!(
        driver.settled_names(),
        vec![String::from("a"), String::from("b"), String::from("c")],
    );
}
