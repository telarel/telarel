use super::*;

#[tokio::test(flavor = "current_thread")]
async fn test_parse_first_some_wins_and_later_hooks_ignored() {
    let ctx: PluginContext<'_> = make_plugin_ctx();

    let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    let plugins: Vec<SharedPluginable> = vec![
        Plugin::new_shared(ParsePlugin::new(
            "first",
            ParseOutcome::Win("let first;"),
            &log,
        )),
        Plugin::new_shared(ParsePlugin::new(
            "second",
            ParseOutcome::Win("let second;"),
            &log,
        )),
    ];

    let driver: PluginDriver = PluginDriver::new(plugins);

    let options: ResolvedParserOptions = parser_options("a.ts");

    let builtin: Parser =
        Parser::new(String::from("let builtin;"), parser_options("a.ts"));

    let result: AstResult =
        driver.parse(&ctx, "let builtin;", &options, &builtin).await.unwrap();

    assert_eq!(result.ast.source().as_ref(), "let first;");

    assert_eq!(*log.lock().unwrap(), vec![String::from("first")]);
}

#[tokio::test(flavor = "current_thread")]
async fn test_parse_null_defer_chain_falls_back_to_builtin() {
    let ctx: PluginContext<'_> = make_plugin_ctx();

    let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    let plugins: Vec<SharedPluginable> = vec![
        Plugin::new_shared(ParsePlugin::new("one", ParseOutcome::Defer, &log)),
        Plugin::new_shared(ParsePlugin::new("two", ParseOutcome::Defer, &log)),
    ];

    let driver: PluginDriver = PluginDriver::new(plugins);

    let options: ResolvedParserOptions = parser_options("a.ts");

    let builtin: Parser =
        Parser::new(String::from("let builtin;"), parser_options("a.ts"));

    let result: AstResult =
        driver.parse(&ctx, "let builtin;", &options, &builtin).await.unwrap();

    assert_eq!(result.ast.source().as_ref(), "let builtin;");

    assert_eq!(
        *log.lock().unwrap(),
        vec![String::from("one"), String::from("two")]
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_parse_error_aborts_and_propagates_plugin_error() {
    let ctx: PluginContext<'_> = make_plugin_ctx();

    let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    let plugins: Vec<SharedPluginable> = vec![
        Plugin::new_shared(ParsePlugin::new("one", ParseOutcome::Defer, &log)),
        Plugin::new_shared(ParsePlugin::new(
            "fail-parse",
            ParseOutcome::Fail,
            &log,
        )),
        Plugin::new_shared(ParsePlugin::new(
            "never",
            ParseOutcome::Win("let never;"),
            &log,
        )),
    ];

    let driver: PluginDriver = PluginDriver::new(plugins);

    let options: ResolvedParserOptions = parser_options("a.ts");

    let builtin: Parser =
        Parser::new(String::from("let builtin;"), parser_options("a.ts"));

    let err: CompileError = driver
        .parse(&ctx, "let builtin;", &options, &builtin)
        .await
        .unwrap_err();

    assert_eq!(err.message(), "fail-parse");

    assert_eq!(
        *log.lock().unwrap(),
        vec![String::from("one"), String::from("fail-parse")]
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_parse_error_preserves_source() {
    let ctx: PluginContext<'_> = make_plugin_ctx();

    let plugins: Vec<SharedPluginable> =
        vec![Plugin::new_shared(SourcedParsePlugin)];

    let driver: PluginDriver = PluginDriver::new(plugins);

    let options: ResolvedParserOptions = parser_options("index.ts");

    let builtin: Parser =
        Parser::new(String::from("const a = 1;"), parser_options("index.ts"));

    let err: CompileError = driver
        .parse(&ctx, "const a = 1;", &options, &builtin)
        .await
        .unwrap_err();

    assert_eq!(err.message(), "boom");

    let source: &telarel_common::ErrorSource =
        err.source().expect("source must survive the boundary");

    assert_eq!(source.file(), "index.ts");
    assert_eq!(source.code(), "const a = 1;");
    assert_eq!(source.span(), oxc::span::Span::new(6, 7));
}

#[tokio::test(flavor = "current_thread")]
async fn test_parse_builtin_fallback_error_is_unwrapped() {
    let ctx: PluginContext<'_> = make_plugin_ctx();

    let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    let plugins: Vec<SharedPluginable> = vec![Plugin::new_shared(
        ParsePlugin::new("defer", ParseOutcome::Defer, &log),
    )];

    let driver: PluginDriver = PluginDriver::new(plugins);

    let options: ResolvedParserOptions = parser_options("index.ts");

    let builtin: Parser =
        Parser::new(String::from("const = ;"), parser_options("index.ts"));

    let err: CompileError =
        driver.parse(&ctx, "const = ;", &options, &builtin).await.unwrap_err();

    // The raw builtin parse error carries the file context, not a
    // plugin-name wrapper.
    assert!(err.to_string().contains("index.ts"), "{err}");

    assert!(!err.to_string().contains("`defer` parse"), "{err}");
}

#[tokio::test(flavor = "current_thread")]
async fn test_parse_skips_plugins_without_usage() {
    let ctx: PluginContext<'_> = make_plugin_ctx();

    let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    // declares Prepare only, yet implements `parse`: the driver must
    // never call it and must fall through to the builtin parser.
    let plugins: Vec<SharedPluginable> =
        vec![Plugin::new_shared(ParseWithOtherUsagePlugin {
            name: "not-parse",
            log: Arc::clone(&log),
        })];

    let driver: PluginDriver = PluginDriver::new(plugins);

    assert_eq!(driver.usage(), HookUsage::Prepare);

    let options: ResolvedParserOptions = parser_options("a.ts");

    let builtin: Parser =
        Parser::new(String::from("let builtin;"), parser_options("a.ts"));

    let result: AstResult =
        driver.parse(&ctx, "let builtin;", &options, &builtin).await.unwrap();

    assert_eq!(result.ast.source().as_ref(), "let builtin;");

    assert!(log.lock().unwrap().is_empty());
}

#[tokio::test(flavor = "current_thread")]
async fn test_parse_hands_code_and_options_to_hooks() {
    let ctx: PluginContext<'_> = make_plugin_ctx();

    let seen: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    let plugins: Vec<SharedPluginable> =
        vec![Plugin::new_shared(ParseObservePlugin {
            seen: Arc::clone(&seen),
        })];

    let driver: PluginDriver = PluginDriver::new(plugins);

    let options: ResolvedParserOptions = parser_options("input.ts");

    let builtin: Parser =
        Parser::new(String::from("let a;"), parser_options("input.ts"));

    driver.parse(&ctx, "let a;", &options, &builtin).await.unwrap();

    assert_eq!(*seen.lock().unwrap(), vec![String::from("let a;|input.ts")]);
}

#[tokio::test(flavor = "current_thread")]
async fn test_generate_first_some_wins_and_later_hooks_ignored() {
    let ctx: PluginContext<'_> = make_plugin_ctx();

    let ast: Ast = test_ast();

    let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    let plugins: Vec<SharedPluginable> = vec![
        Plugin::new_shared(GeneratePlugin::new(
            "first",
            GenerateOutcome::Win("first;"),
            &log,
        )),
        Plugin::new_shared(GeneratePlugin::new(
            "second",
            GenerateOutcome::Win("second;"),
            &log,
        )),
    ];

    let driver: PluginDriver = PluginDriver::new(plugins);

    let options: ResolvedGeneratorOptions = generator_options("a.ts");

    let builtin: Generator<'_> =
        Generator::new(&ast, generator_options("a.ts"));

    let result: CodeResult =
        driver.generate(&ctx, &ast, &options, &builtin).await.unwrap();

    assert_eq!(result.code, "first;");

    assert_eq!(*log.lock().unwrap(), vec![String::from("first")]);
}

#[tokio::test(flavor = "current_thread")]
async fn test_generate_null_defer_chain_falls_back_to_builtin() {
    let ctx: PluginContext<'_> = make_plugin_ctx();

    let ast: Ast = test_ast();

    let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    let plugins: Vec<SharedPluginable> = vec![
        Plugin::new_shared(GeneratePlugin::new(
            "one",
            GenerateOutcome::Defer,
            &log,
        )),
        Plugin::new_shared(GeneratePlugin::new(
            "two",
            GenerateOutcome::Defer,
            &log,
        )),
    ];

    let driver: PluginDriver = PluginDriver::new(plugins);

    let options: ResolvedGeneratorOptions = generator_options("a.ts");

    let builtin: Generator<'_> =
        Generator::new(&ast, generator_options("a.ts"));

    let result: CodeResult =
        driver.generate(&ctx, &ast, &options, &builtin).await.unwrap();

    assert_eq!(result.code, "console.log(1);");

    assert!(result.map.is_some(), "codegen produces a real map");

    assert_eq!(
        *log.lock().unwrap(),
        vec![String::from("one"), String::from("two")]
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_generate_error_aborts_and_propagates_plugin_error() {
    let ctx: PluginContext<'_> = make_plugin_ctx();

    let ast: Ast = test_ast();

    let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    let plugins: Vec<SharedPluginable> = vec![
        Plugin::new_shared(GeneratePlugin::new(
            "one",
            GenerateOutcome::Defer,
            &log,
        )),
        Plugin::new_shared(GeneratePlugin::new(
            "fail-generate",
            GenerateOutcome::Fail,
            &log,
        )),
    ];

    let driver: PluginDriver = PluginDriver::new(plugins);

    let options: ResolvedGeneratorOptions = generator_options("a.ts");

    let builtin: Generator<'_> =
        Generator::new(&ast, generator_options("a.ts"));

    let err: CompileError =
        driver.generate(&ctx, &ast, &options, &builtin).await.unwrap_err();

    assert_eq!(err.message(), "fail-generate");
}

#[tokio::test(flavor = "current_thread")]
async fn test_generate_hands_ast_and_options_to_hooks() {
    let ctx: PluginContext<'_> = make_plugin_ctx();

    let ast: Ast = test_ast();

    let seen: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    let plugins: Vec<SharedPluginable> =
        vec![Plugin::new_shared(GenerateObservePlugin {
            seen: Arc::clone(&seen),
        })];

    let driver: PluginDriver = PluginDriver::new(plugins);

    let options: ResolvedGeneratorOptions = generator_options("input.ts");

    let builtin: Generator<'_> =
        Generator::new(&ast, generator_options("input.ts"));

    driver.generate(&ctx, &ast, &options, &builtin).await.unwrap();

    assert_eq!(
        *seen.lock().unwrap(),
        vec![String::from("console.log(1);|input.ts")]
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_prepare_runs_all_plugins_in_order() {
    let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    let plugins: Vec<SharedPluginable> = vec![
        Plugin::new_shared(ObserverPlugin {
            name: "first",
            usage: HookUsage::Prepare | HookUsage::Finalize,
            fail: false,
            log: Arc::clone(&log),
        }),
        Plugin::new_shared(ObserverPlugin {
            name: "second",
            usage: HookUsage::Prepare | HookUsage::Finalize,
            fail: false,
            log: Arc::clone(&log),
        }),
    ];

    let driver: PluginDriver = PluginDriver::new(plugins);

    let ctx: PluginContext<'_> = make_plugin_ctx();

    let ast: Ast = test_ast();

    driver.prepare(&ctx, PrepareArgs { ast: &ast }).await.unwrap();

    assert_eq!(
        *log.lock().unwrap(),
        vec![String::from("first"), String::from("second")]
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_prepare_error_aborts_and_propagates_plugin_error() {
    let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    let plugins: Vec<SharedPluginable> = vec![
        Plugin::new_shared(ObserverPlugin {
            name: "first",
            usage: HookUsage::Prepare,
            fail: false,
            log: Arc::clone(&log),
        }),
        Plugin::new_shared(ObserverPlugin {
            name: "fail-prepare",
            usage: HookUsage::Prepare,
            fail: true,
            log: Arc::clone(&log),
        }),
        Plugin::new_shared(ObserverPlugin {
            name: "never",
            usage: HookUsage::Prepare,
            fail: false,
            log: Arc::clone(&log),
        }),
    ];

    let driver: PluginDriver = PluginDriver::new(plugins);

    let ctx: PluginContext<'_> = make_plugin_ctx();

    let ast: Ast = test_ast();

    let err: PluginError =
        driver.prepare(&ctx, PrepareArgs { ast: &ast }).await.unwrap_err();

    assert_eq!(err.message(), "fail-prepare");

    assert_eq!(
        *log.lock().unwrap(),
        vec![String::from("first"), String::from("fail-prepare")]
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_prepare_observers_receive_read_only_borrow() {
    let ctx: PluginContext<'_> = make_plugin_ctx();

    let ast: Ast = test_ast();

    let seen: Arc<Mutex<Vec<usize>>> = Arc::new(Mutex::new(Vec::new()));

    let plugins: Vec<SharedPluginable> =
        vec![Plugin::new_shared(ObserveAstPlugin {
            usage: HookUsage::Prepare,
            seen: Arc::clone(&seen),
        })];

    let driver: PluginDriver = PluginDriver::new(plugins);

    driver.prepare(&ctx, PrepareArgs { ast: &ast }).await.unwrap();

    assert_eq!(
        *seen.lock().unwrap(),
        vec![std::ptr::from_ref(ast.program()) as usize],
        "prepare must observe the same AST by reference"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_finalize_runs_all_plugins_in_order() {
    let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    let plugins: Vec<SharedPluginable> = vec![
        Plugin::new_shared(ObserverPlugin {
            name: "first",
            usage: HookUsage::Prepare | HookUsage::Finalize,
            fail: false,
            log: Arc::clone(&log),
        }),
        Plugin::new_shared(ObserverPlugin {
            name: "second",
            usage: HookUsage::Prepare | HookUsage::Finalize,
            fail: false,
            log: Arc::clone(&log),
        }),
    ];

    let driver: PluginDriver = PluginDriver::new(plugins);

    let ctx: PluginContext<'_> = make_plugin_ctx();

    let ast: Ast = test_ast();

    driver.finalize(&ctx, FinalizeArgs { ast: &ast }).await.unwrap();

    assert_eq!(
        *log.lock().unwrap(),
        vec![String::from("first"), String::from("second")]
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_finalize_error_propagates_plugin_error() {
    let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    let plugins: Vec<SharedPluginable> = vec![
        Plugin::new_shared(ObserverPlugin {
            name: "first",
            usage: HookUsage::Finalize,
            fail: false,
            log: Arc::clone(&log),
        }),
        Plugin::new_shared(ObserverPlugin {
            name: "fail-finalize",
            usage: HookUsage::Finalize,
            fail: true,
            log: Arc::clone(&log),
        }),
    ];

    let driver: PluginDriver = PluginDriver::new(plugins);

    let ctx: PluginContext<'_> = make_plugin_ctx();

    let ast: Ast = test_ast();

    let err: PluginError =
        driver.finalize(&ctx, FinalizeArgs { ast: &ast }).await.unwrap_err();

    assert_eq!(err.message(), "fail-finalize");
}

#[tokio::test(flavor = "current_thread")]
async fn test_finalize_observers_receive_read_only_borrow() {
    let ctx: PluginContext<'_> = make_plugin_ctx();

    let ast: Ast = test_ast();

    let seen: Arc<Mutex<Vec<usize>>> = Arc::new(Mutex::new(Vec::new()));

    let plugins: Vec<SharedPluginable> =
        vec![Plugin::new_shared(ObserveAstPlugin {
            usage: HookUsage::Finalize,
            seen: Arc::clone(&seen),
        })];

    let driver: PluginDriver = PluginDriver::new(plugins);

    driver.finalize(&ctx, FinalizeArgs { ast: &ast }).await.unwrap();

    assert_eq!(
        *seen.lock().unwrap(),
        vec![std::ptr::from_ref(ast.program()) as usize],
        "finalize must observe the same AST by reference"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_transform_chains_programs() {
    let ctx: PluginContext<'_> = make_plugin_ctx();

    let ast: Ast = test_ast();

    let args: TransformArgs = TransformArgs { ast: ast.clone() };

    let plugins: Vec<SharedPluginable> =
        vec![Plugin::new_shared(MarkPlugin), Plugin::new_shared(MarkPlugin)];

    let driver: PluginDriver = PluginDriver::new(plugins);

    let output: Option<crate::TransformOutput> =
        driver.transform(&ctx, args).await.unwrap();

    let output: crate::TransformOutput =
        output.expect("the chain changed the program");

    let out: String =
        oxc::codegen::Codegen::new().build(output.ast.program()).code;

    assert!(out.contains("mark"), "{out}");
}

#[tokio::test(flavor = "current_thread")]
async fn test_transform_error_propagates_plugin_error() {
    let ctx: PluginContext<'_> = make_plugin_ctx();

    let ast: Ast = test_ast();

    let args: TransformArgs = TransformArgs { ast: ast.clone() };

    let plugins: Vec<SharedPluginable> =
        vec![Plugin::new_shared(FailingTransformPlugin)];

    let driver: PluginDriver = PluginDriver::new(plugins);

    let err: PluginError = driver.transform(&ctx, args).await.unwrap_err();

    assert_eq!(err.message(), "boom");
}

#[tokio::test(flavor = "current_thread")]
async fn test_transform_error_preserves_source() {
    let ctx: PluginContext<'_> = make_plugin_ctx();

    let ast: Ast = test_ast();

    let args: TransformArgs = TransformArgs { ast: ast.clone() };

    let plugins: Vec<SharedPluginable> =
        vec![Plugin::new_shared(SourcedTransformPlugin)];

    let driver: PluginDriver = PluginDriver::new(plugins);

    let err: PluginError = driver.transform(&ctx, args).await.unwrap_err();

    assert_eq!(err.message(), "boom");

    let source: &telarel_common::ErrorSource =
        err.source().expect("source must survive the driver boundary");

    assert_eq!(source.file(), "index.ts");
    assert_eq!(source.code(), "const a = 1;");
    assert_eq!(source.span(), oxc::span::Span::new(6, 7));
}

#[tokio::test(flavor = "current_thread")]
async fn test_transform_shares_the_original_arena() {
    let ctx: PluginContext<'_> = make_plugin_ctx();

    let ast: Ast = test_ast();

    let args: TransformArgs = TransformArgs { ast: ast.clone() };

    let seen: Arc<Mutex<Vec<usize>>> = Arc::new(Mutex::new(Vec::new()));

    let plugins: Vec<SharedPluginable> =
        vec![Plugin::new_shared(RecordingPlugin::new(Arc::clone(&seen)))];

    let driver: PluginDriver = PluginDriver::new(plugins);

    let output: Option<crate::TransformOutput> =
        driver.transform(&ctx, args).await.unwrap();

    assert!(output.is_none());

    let recorded: Vec<usize> = seen.lock().unwrap().clone();

    // No plugin returned `Some`, so the fold hands the hook an O(1) clone
    // that shares the original arena instead of rebuilding it.
    assert_eq!(recorded, vec![std::ptr::from_ref(ast.program()) as usize],);
}

#[tokio::test(flavor = "current_thread")]
async fn test_transform_some_program_reaches_later_plugins() {
    // A `Some`-returning plugin's AST is what the following plugin
    // receives AND what the fold returns. The regression this pins: a
    // fold that kept handing later plugins the original AST.
    let ctx: PluginContext<'_> = make_plugin_ctx();

    let ast: Ast = test_ast();

    let args: TransformArgs = TransformArgs { ast: ast.clone() };

    let seen: Arc<Mutex<Vec<usize>>> = Arc::new(Mutex::new(Vec::new()));

    let plugins: Vec<SharedPluginable> = vec![
        Plugin::new_shared(ReplaceRootPlugin),
        Plugin::new_shared(RecordingPlugin::new(Arc::clone(&seen))),
    ];

    let driver: PluginDriver = PluginDriver::new(plugins);

    let output: Option<crate::TransformOutput> =
        driver.transform(&ctx, args).await.unwrap();

    let output: crate::TransformOutput =
        output.expect("the chain changed the program");

    let recorded: usize = seen.lock().unwrap()[0];

    assert_eq!(
        recorded,
        std::ptr::from_ref(output.ast.program()) as usize,
        "later plugin must receive the Some-returned AST"
    );

    let out: String =
        oxc::codegen::Codegen::new().build(output.ast.program()).code;

    assert!(out.contains("replaced"), "{out}");
}

#[tokio::test(flavor = "current_thread")]
async fn test_transform_all_none_returns_none() {
    let ctx: PluginContext<'_> = make_plugin_ctx();

    let ast: Ast = test_ast();

    let args: TransformArgs = TransformArgs { ast: ast.clone() };

    let first_seen: Arc<Mutex<Vec<usize>>> = Arc::new(Mutex::new(Vec::new()));

    let second_seen: Arc<Mutex<Vec<usize>>> = Arc::new(Mutex::new(Vec::new()));

    let plugins: Vec<SharedPluginable> = vec![
        Plugin::new_shared(RecordingPlugin::new(Arc::clone(&first_seen))),
        Plugin::new_shared(RecordingPlugin::new(Arc::clone(&second_seen))),
    ];

    let driver: PluginDriver = PluginDriver::new(plugins);

    let output: Option<crate::TransformOutput> =
        driver.transform(&ctx, args).await.unwrap();

    assert!(output.is_none());

    let original: usize = std::ptr::from_ref(ast.program()) as usize;

    assert_eq!(first_seen.lock().unwrap()[0], original);

    assert_eq!(second_seen.lock().unwrap()[0], original);
}

#[tokio::test(flavor = "current_thread")]
async fn test_transform_last_some_wins() {
    let ctx: PluginContext<'_> = make_plugin_ctx();

    let ast: Ast = test_ast();

    let args: TransformArgs = TransformArgs { ast: ast.clone() };

    let plugins: Vec<SharedPluginable> = vec![
        Plugin::new_shared(ReplaceRootPlugin),
        Plugin::new_shared(MarkPlugin),
    ];

    let driver: PluginDriver = PluginDriver::new(plugins);

    let output: Option<crate::TransformOutput> =
        driver.transform(&ctx, args).await.unwrap();

    let output: crate::TransformOutput =
        output.expect("the chain changed the program");

    let out: String =
        oxc::codegen::Codegen::new().build(output.ast.program()).code;

    assert!(out.contains("mark"), "{out}");

    assert!(!out.contains("replaced"), "{out}");
}

#[tokio::test(flavor = "current_thread")]
async fn test_transform_some_then_second_some_keeps_both_edits() {
    let ctx: PluginContext<'_> = make_plugin_ctx();

    let ast: Ast = test_ast();

    let args: TransformArgs = TransformArgs { ast: ast.clone() };

    let plugins: Vec<SharedPluginable> = vec![
        Plugin::new_shared(ReplaceRootPlugin),
        Plugin::new_shared(AppendDirectivePlugin),
    ];

    let driver: PluginDriver = PluginDriver::new(plugins);

    let output: Option<crate::TransformOutput> =
        driver.transform(&ctx, args).await.unwrap();

    let output: crate::TransformOutput =
        output.expect("the chain changed the program");

    let out: String =
        oxc::codegen::Codegen::new().build(output.ast.program()).code;

    assert!(out.contains("replaced"), "{out}");

    assert!(out.contains(APPEND_DIRECTIVE), "{out}");
}

#[tokio::test(flavor = "current_thread")]
async fn test_transform_skips_plugins_without_usage() {
    let ctx: PluginContext<'_> = make_plugin_ctx();

    let ast: Ast = test_ast();

    let args: TransformArgs = TransformArgs { ast: ast.clone() };

    let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    let plugins: Vec<SharedPluginable> =
        vec![Plugin::new_shared(ObserverPlugin {
            name: "prepare-only",
            usage: HookUsage::Prepare,
            fail: false,
            log: Arc::clone(&log),
        })];

    let driver: PluginDriver = PluginDriver::new(plugins);

    driver.transform(&ctx, args).await.unwrap();

    let recorded: Vec<String> = log.lock().unwrap().clone();

    assert_eq!(driver.usage(), HookUsage::Prepare);
    assert!(recorded.is_empty());
}

fn probe_members(probe: &BatchProbe) -> Vec<Vec<String>> {
    probe.members.lock().unwrap().clone()
}

fn member_names(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| name.to_string()).collect()
}

#[tokio::test(flavor = "current_thread")]
async fn test_transform_batches_adjacent_batchable_plugins() {
    let ctx: PluginContext<'_> = make_plugin_ctx();

    let probe: Arc<BatchProbe> = Arc::new(BatchProbe::default());

    let plugins: Vec<SharedPluginable> = vec![
        probe.batchable("one", None, BatchOutcome::None),
        probe.batchable("two", None, BatchOutcome::None),
    ];

    let driver: PluginDriver = PluginDriver::new(plugins);

    let output: Option<crate::TransformOutput> = driver
        .transform(&ctx, TransformArgs { ast: test_ast() })
        .await
        .unwrap();

    assert!(output.is_none());
    assert_eq!(probe_counts(&probe), (0, 1, 1));
    assert_eq!(probe_members(&probe), vec![member_names(&["one", "two"])]);
}

#[tokio::test(flavor = "current_thread")]
async fn test_transform_non_batchable_plugin_splits_batch_runs() {
    let ctx: PluginContext<'_> = make_plugin_ctx();

    let probe: Arc<BatchProbe> = Arc::new(BatchProbe::default());

    let plugins: Vec<SharedPluginable> = vec![
        probe.batchable("a1", None, BatchOutcome::None),
        probe.batchable("a2", None, BatchOutcome::None),
        probe.plain("middle", None),
        probe.batchable("b1", None, BatchOutcome::None),
        probe.batchable("b2", None, BatchOutcome::None),
    ];

    let driver: PluginDriver = PluginDriver::new(plugins);

    let output: Option<crate::TransformOutput> = driver
        .transform(&ctx, TransformArgs { ast: test_ast() })
        .await
        .unwrap();

    assert!(output.is_none());
    assert_eq!(probe_counts(&probe), (1, 2, 2));
    assert_eq!(
        probe_members(&probe),
        vec![member_names(&["a1", "a2"]), member_names(&["b1", "b2"])]
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_transform_batch_none_keeps_carried_ast() {
    let ctx: PluginContext<'_> = make_plugin_ctx();

    let probe: Arc<BatchProbe> = Arc::new(BatchProbe::default());

    let ast: Ast = test_ast();

    let plugins: Vec<SharedPluginable> = vec![
        probe.batchable("one", None, BatchOutcome::None),
        probe.batchable("two", None, BatchOutcome::None),
    ];

    let driver: PluginDriver = PluginDriver::new(plugins);

    let output: Option<crate::TransformOutput> = driver
        .transform(&ctx, TransformArgs { ast: ast.clone() })
        .await
        .unwrap();

    assert!(output.is_none());
    assert_eq!(probe_counts(&probe), (0, 1, 1));

    let out: String = oxc::codegen::Codegen::new().build(ast.program()).code;

    assert!(out.contains("console.log"), "{out}");
}

#[tokio::test(flavor = "current_thread")]
async fn test_transform_batch_none_after_change_keeps_result() {
    let ctx: PluginContext<'_> = make_plugin_ctx();

    let probe: Arc<BatchProbe> = Arc::new(BatchProbe::default());

    let seen: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    let plugins: Vec<SharedPluginable> = vec![
        probe.batchable(
            "change-1",
            None,
            BatchOutcome::Replace("\"accepted\";"),
        ),
        probe.batchable("change-2", None, BatchOutcome::None),
        probe.plain("separator", None),
        probe.batchable("none-1", None, BatchOutcome::None),
        probe.batchable("none-2", None, BatchOutcome::None),
        Plugin::new_shared(ObserveCarriedAstPlugin { seen: Arc::clone(&seen) }),
    ];

    let driver: PluginDriver = PluginDriver::new(plugins);

    let output: crate::TransformOutput = driver
        .transform(&ctx, TransformArgs { ast: test_ast() })
        .await
        .unwrap()
        .expect("the earlier accepted change remains");

    let out: String =
        oxc::codegen::Codegen::new().build(output.ast.program()).code;

    assert!(out.contains("accepted"), "{out}");
    assert!(!out.contains("console.log"), "{out}");

    assert_eq!(*seen.lock().unwrap(), vec![String::from("\"accepted\";")]);

    assert_eq!(
        probe_members(&probe),
        vec![
            member_names(&["change-1", "change-2"]),
            member_names(&["none-1", "none-2"])
        ]
    );

    assert_eq!(probe_counts(&probe), (1, 2, 2));
}

#[tokio::test(flavor = "current_thread")]
async fn test_transform_batch_some_replaces_carried_ast() {
    let ctx: PluginContext<'_> = make_plugin_ctx();

    let probe: Arc<BatchProbe> = Arc::new(BatchProbe::default());

    let plugins: Vec<SharedPluginable> = vec![
        probe.batchable("one", None, BatchOutcome::Replace("\"batched\";")),
        probe.batchable("two", None, BatchOutcome::None),
    ];

    let driver: PluginDriver = PluginDriver::new(plugins);

    let output: crate::TransformOutput = driver
        .transform(&ctx, TransformArgs { ast: test_ast() })
        .await
        .unwrap()
        .expect("the batch changed the program");

    let out: String =
        oxc::codegen::Codegen::new().build(output.ast.program()).code;

    assert!(out.contains("batched"), "{out}");
    assert!(!out.contains("console.log"), "{out}");
}

#[tokio::test(flavor = "current_thread")]
async fn test_transform_batch_error_propagates() {
    let ctx: PluginContext<'_> = make_plugin_ctx();

    let probe: Arc<BatchProbe> = Arc::new(BatchProbe::default());

    let plugins: Vec<SharedPluginable> = vec![
        probe.batchable("one", None, BatchOutcome::Fail),
        probe.batchable("two", None, BatchOutcome::None),
    ];

    let driver: PluginDriver = PluginDriver::new(plugins);

    let error: PluginError = driver
        .transform(&ctx, TransformArgs { ast: test_ast() })
        .await
        .unwrap_err();

    assert_eq!(error.message(), "batch-boom");
    assert_eq!(probe_counts(&probe), (0, 1, 1));
}

#[tokio::test(flavor = "current_thread")]
async fn test_transform_batch_preserves_pre_normal_post_order() {
    let ctx: PluginContext<'_> = make_plugin_ctx();

    let probe: Arc<BatchProbe> = Arc::new(BatchProbe::default());

    let pre: PluginHookMeta = PluginHookMeta { order: Some(PluginOrder::Pre) };

    let post: PluginHookMeta =
        PluginHookMeta { order: Some(PluginOrder::Post) };

    let plugins: Vec<SharedPluginable> = vec![
        probe.batchable("normal", None, BatchOutcome::None),
        probe.batchable("post", Some(post), BatchOutcome::None),
        probe.batchable("pre", Some(pre), BatchOutcome::None),
    ];

    let driver: PluginDriver = PluginDriver::new(plugins);

    driver.transform(&ctx, TransformArgs { ast: test_ast() }).await.unwrap();

    assert_eq!(
        probe_members(&probe),
        vec![member_names(&["pre", "normal", "post"])]
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_transform_single_batchable_uses_single_path() {
    let ctx: PluginContext<'_> = make_plugin_ctx();

    let probe: Arc<BatchProbe> = Arc::new(BatchProbe::default());

    let plugins: Vec<SharedPluginable> = vec![probe.batchable(
        "only",
        None,
        BatchOutcome::Replace("\"ignored\";"),
    )];

    let driver: PluginDriver = PluginDriver::new(plugins);

    let output: Option<crate::TransformOutput> = driver
        .transform(&ctx, TransformArgs { ast: test_ast() })
        .await
        .unwrap();

    assert!(output.is_none());
    assert_eq!(probe_counts(&probe), (1, 0, 0));
    assert!(probe_members(&probe).is_empty());
}

#[tokio::test(flavor = "current_thread")]
async fn test_transform_batch_none_builder_falls_back_to_single_calls() {
    let ctx: PluginContext<'_> = make_plugin_ctx();

    let probe: Arc<BatchProbe> = Arc::new(BatchProbe::default());

    let plugins: Vec<SharedPluginable> = vec![
        probe.batchable_without_batch("one", None),
        probe.batchable_without_batch("two", None),
    ];

    let driver: PluginDriver = PluginDriver::new(plugins);

    let output: Option<crate::TransformOutput> = driver
        .transform(&ctx, TransformArgs { ast: test_ast() })
        .await
        .unwrap();

    assert!(output.is_none());
    assert_eq!(probe_counts(&probe), (2, 1, 0));
    assert_eq!(probe_members(&probe), vec![member_names(&["one", "two"])]);
}

#[tokio::test(flavor = "current_thread")]
async fn test_pluginable_dispatch() {
    let plugin: Arc<dyn Pluginable> = Plugin::new_shared(MarkPlugin);

    assert_eq!(plugin.call_name(), "mark");

    assert!(plugin.call_register_hook_usage().contains(HookUsage::Transform));
}
