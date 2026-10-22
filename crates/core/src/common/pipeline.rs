//! The shared pipeline skeleton behind the five public entry points.
//!
//! Every entry point converts its options to [`ResolvableOptions`], runs the
//! `options` fixpoint exactly once, resolves the settled options, runs `start`
//! exactly once, executes the stages its [`Stage`] declares, then runs `end`
//! exactly once. Stages are run inline — never by re-entering another public
//! function — so `options`, `start`, and `end` fire once per invocation. The
//! only path that skips `end` is an `options`-fixpoint failure, where no
//! driver exists yet.

use telarel_common::{
    Ast, AstResult, CodeResult, CompileError, Generator, GeneratorOptions,
    OutputSourceMap, OutputSourceMapOptions, Parser, ParserOptions,
    ResolvableOptions, ResolvedOptions, apply_output_source_map_mode,
    resolve_input_source_map, resolve_options,
};
use telarel_plugin::__internal::PluginDriver;
use telarel_plugin::{
    CommonPluginContext, EndArgs, FinalizeArgs, ModuleInfo, OptionsArgs,
    PluginContext, PluginError, PrepareArgs, SharedPluginable, StartArgs,
    TransformArgs, TransformOutput, options_fixpoint,
};

/// The stage plan of one public entry point.
pub trait Stage {
    type Output;

    /// Run this entry point's configured stages over the settled pipeline.
    async fn run(
        self,
        driver: &PluginDriver,
        ctx: &PluginContext<'_>,
        resolved: &ResolvedOptions,
    ) -> Result<Self::Output, CompileError>;
}

/// The resolved output options; `None` falls back to the defaults.
fn output_options(output: Option<&OutputSourceMap>) -> OutputSourceMapOptions {
    match output {
        | Some(OutputSourceMap::Mode(mode)) => {
            OutputSourceMapOptions { mode: *mode, url: None }
        },
        | Some(OutputSourceMap::Options(options)) => options.clone(),
        | None => OutputSourceMapOptions::default(),
    }
}

/// Attach the resolved input map to `ast` when it carries none.
///
/// The AST's own map always wins; `Ok(None)` from the resolver means there is
/// nothing to attach.
fn attach_input_map(
    ast: Ast,
    resolved: &ResolvedOptions,
) -> Result<Ast, CompileError> {
    if ast.map().is_some() {
        return Ok(ast);
    }

    let Some(input) = resolved.source_map.input.as_ref() else {
        return Ok(ast);
    };

    match resolve_input_source_map(
        ast.source(),
        &resolved.cwd,
        &resolved.file,
        input,
    )? {
        | Some(map) => Ok(ast.with_input_map(Some(map))),
        | None => Ok(ast),
    }
}

/// The `parse` stage: the builtin parser is the terminal fallback after every
/// `parse` hook defers, and it resolves the input source map itself.
async fn parse_stage(
    driver: &PluginDriver,
    ctx: &PluginContext<'_>,
    resolved: &ResolvedOptions,
    code: &str,
) -> Result<AstResult, CompileError> {
    let options: ParserOptions = ParserOptions::from(resolved);

    let builtin: Parser = Parser::new(code.to_string(), options.clone());

    driver.parse(ctx, code, &options, &builtin).await
}

async fn rewrite_stages(
    driver: &PluginDriver,
    ctx: &PluginContext<'_>,
    ast: Ast,
) -> Result<Ast, CompileError> {
    driver
        .prepare(ctx, PrepareArgs { ast: &ast })
        .await
        .map_err(|error: PluginError| error.into_compile_error())?;

    let transformed: Option<TransformOutput> = driver
        .transform(ctx, TransformArgs { ast: ast.clone() })
        .await
        .map_err(|error: PluginError| error.into_compile_error())?;

    let ast: Ast = match transformed {
        | Some(output) => output.ast,
        | None => ast,
    };

    driver
        .finalize(ctx, FinalizeArgs { ast: &ast })
        .await
        .map_err(|error: PluginError| error.into_compile_error())?;

    Ok(ast)
}

/// The `generate` stage: the builtin generator is the terminal fallback after
/// every `generate` hook defers. The resolved output source-map options are
/// applied uniformly, regardless of which hook won.
async fn generate_stage(
    driver: &PluginDriver,
    ctx: &PluginContext<'_>,
    resolved: &ResolvedOptions,
    ast: &Ast,
) -> Result<CodeResult, CompileError> {
    let options: GeneratorOptions = GeneratorOptions::from(resolved);

    let builtin: Generator<'_> = Generator::new(ast, options.clone());

    let result: CodeResult =
        driver.generate(ctx, ast, &options, &builtin).await?;

    let output: Option<&OutputSourceMap> = resolved.source_map.output.as_ref();
    let output_file: Option<&str> =
        output.map(|_output: &OutputSourceMap| resolved.output.file.as_str());

    Ok(apply_output_source_map_mode(
        result,
        &output_options(output),
        output_file,
    ))
}

/// Run `end` around a stage result.
///
/// On success `error` is unset; on a stage failure it carries the original
/// stage error. A failing `end` replaces the returned error with its own; a
/// succeeding `end` leaves the stage error untouched. `end` itself runs on
/// every path after the settle.
async fn finish<T>(
    driver: &PluginDriver,
    ctx: &PluginContext<'_>,
    result: Result<T, CompileError>,
) -> Result<T, CompileError> {
    match result {
        | Ok(output) => {
            driver
                .end(ctx, &EndArgs { error: None })
                .await
                .map_err(|error: PluginError| error.into_compile_error())?;

            Ok(output)
        },
        | Err(error) => {
            let mut args: EndArgs = EndArgs { error: Some(error) };

            match driver.end(ctx, &args).await {
                | Ok(()) => {
                    Err(args.error.take().expect("stage error present"))
                },
                | Err(end_error) => Err(end_error.into_compile_error()),
            }
        },
    }
}

pub struct ParseStage {
    pub code: String,
}

impl Stage for ParseStage {
    type Output = AstResult;

    async fn run(
        self,
        driver: &PluginDriver,
        ctx: &PluginContext<'_>,
        resolved: &ResolvedOptions,
    ) -> Result<AstResult, CompileError> {
        parse_stage(driver, ctx, resolved, &self.code).await
    }
}

pub struct RewriteStage {
    pub ast: Ast,
}

impl Stage for RewriteStage {
    type Output = AstResult;

    async fn run(
        self,
        driver: &PluginDriver,
        ctx: &PluginContext<'_>,
        resolved: &ResolvedOptions,
    ) -> Result<AstResult, CompileError> {
        let ast: Ast = attach_input_map(self.ast, resolved)?;

        let ast: Ast = rewrite_stages(driver, ctx, ast).await?;

        Ok(AstResult { ast })
    }
}

pub struct GenerateStage {
    pub ast: Ast,
}

impl Stage for GenerateStage {
    type Output = CodeResult;

    async fn run(
        self,
        driver: &PluginDriver,
        ctx: &PluginContext<'_>,
        resolved: &ResolvedOptions,
    ) -> Result<CodeResult, CompileError> {
        let ast: Ast = attach_input_map(self.ast, resolved)?;

        generate_stage(driver, ctx, resolved, &ast).await
    }
}

pub struct CompileFromAstStage {
    pub ast: Ast,
}

impl Stage for CompileFromAstStage {
    type Output = CodeResult;

    async fn run(
        self,
        driver: &PluginDriver,
        ctx: &PluginContext<'_>,
        resolved: &ResolvedOptions,
    ) -> Result<CodeResult, CompileError> {
        let ast: Ast = attach_input_map(self.ast, resolved)?;

        let ast: Ast = rewrite_stages(driver, ctx, ast).await?;

        generate_stage(driver, ctx, resolved, &ast).await
    }
}

pub struct CompileStage {
    pub code: String,
}

impl Stage for CompileStage {
    type Output = CodeResult;

    async fn run(
        self,
        driver: &PluginDriver,
        ctx: &PluginContext<'_>,
        resolved: &ResolvedOptions,
    ) -> Result<CodeResult, CompileError> {
        let ast: AstResult =
            parse_stage(driver, ctx, resolved, &self.code).await?;

        let ast: Ast = rewrite_stages(driver, ctx, ast.ast).await?;

        generate_stage(driver, ctx, resolved, &ast).await
    }
}

/// Run the shared skeleton around `stage`.
pub async fn run<S: Stage>(
    stage: S,
    options: ResolvableOptions,
    plugins: Vec<SharedPluginable>,
) -> Result<S::Output, CompileError> {
    // The `options` fixpoint runs BEFORE any driver exists; plugins injected
    // into the args join the settled list.
    let common: CommonPluginContext = CommonPluginContext::default();

    let args: OptionsArgs = OptionsArgs { options, plugins };

    let (args, settled): (OptionsArgs, Vec<SharedPluginable>) =
        options_fixpoint(&common, &args)
            .await
            .map_err(|error: PluginError| error.into_compile_error())?;

    // The driver is built from the SETTLED list: its usage aggregate covers
    // plugins injected during the `options` stage, and its settled names feed
    // resolution.
    let driver: PluginDriver = PluginDriver::new(settled);

    let resolved: ResolvedOptions =
        resolve_options(args.options, driver.settled_names().to_vec());

    let module: ModuleInfo<'_> = ModuleInfo {
        file: &resolved.file,
        language: resolved.language,
        source_type: resolved.source_type,
    };

    let ctx: PluginContext<'_> = PluginContext::new(&resolved.cwd, module);

    // A `start` failure is a stage failure: it flows into `finish` so `end`
    // still runs. Only the options-fixpoint failure (above) skips `end`.
    let result: Result<S::Output, CompileError> = match driver
        .start(&ctx, &StartArgs { options: resolved.clone() })
        .await
        .map_err(|error: PluginError| error.into_compile_error())
    {
        | Ok(()) => stage.run(&driver, &ctx, &resolved).await,
        | Err(error) => Err(error),
    };

    finish(&driver, &ctx, result).await
}

#[cfg(test)]
#[allow(clippy::manual_async_fn)]
mod tests {
    use std::borrow::Cow;
    use std::future::Future;
    use std::sync::Arc;
    use std::sync::Mutex;

    use oxc::span::Span;
    use telarel_common::{
        Ast, CompileError, CompileOptions, ErrorSource, Language, ParseOptions,
        ResolvedOptions, RewriteOptions, SourceType,
    };
    use telarel_plugin::{
        EndArgs, HookUsage, NotifyReturn, ParseArgs, ParseReturn, Plugin,
        PluginContext, PluginError, SharedPluginable, TransformArgs,
        TransformReturn,
    };

    use crate::common::test_util::{
        EndObserverPlugin, FailPlugin, FailStage, StartObserverPlugin,
        parse_fixture,
    };
    use crate::functions::compile::compile;
    use crate::functions::parse::parse;
    use crate::functions::rewrite::rewrite;

    #[derive(Debug)]
    struct SourceFailParsePlugin;

    impl Plugin for SourceFailParsePlugin {
        fn name(&self) -> Cow<'static, str> {
            "source-parse".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Parse
        }

        fn parse<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            _args: ParseArgs<'a>,
        ) -> impl Future<Output = ParseReturn> + Send + 'a {
            async move {
                Err(PluginError::new("boom").with_source_code(
                    "input.ts",
                    "const x = 1;",
                    Span::new(6, 7),
                ))
            }
        }
    }

    #[derive(Debug)]
    struct SourceFailEndPlugin;

    impl Plugin for SourceFailEndPlugin {
        fn name(&self) -> Cow<'static, str> {
            "source-end".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::End
        }

        fn end<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            _args: &'a EndArgs,
        ) -> impl Future<Output = NotifyReturn> + Send {
            async move {
                Err(PluginError::new("boom").with_source_code(
                    "end.ts",
                    "end();",
                    Span::new(0, 3),
                ))
            }
        }
    }

    #[derive(Debug)]
    struct SourceFailTransformPlugin;

    impl Plugin for SourceFailTransformPlugin {
        fn name(&self) -> Cow<'static, str> {
            "source-transform".into()
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
                Err(PluginError::new("boom").with_source_code(
                    "input.ts",
                    "const x = 1;",
                    Span::new(6, 7),
                ))
            }
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_end_failure_replaces_the_stage_error() {
        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(FailPlugin::new("fail-start", FailStage::Start)),
            Plugin::new_shared(FailPlugin::new("fail-end", FailStage::End)),
        ];

        let error: CompileError = parse(
            String::from("const a = 1;"),
            ParseOptions::default(),
            plugins,
        )
        .await
        .expect_err("the stage failure fails the invocation");

        assert_eq!(error.message(), "fail-end");

        let rendered: String = error.to_string();

        assert!(!rendered.contains("fail-start"), "{rendered}");
        assert!(!rendered.contains("end hook failed"), "{rendered}");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_end_error_on_success_path_aborts() {
        let plugins: Vec<SharedPluginable> = vec![Plugin::new_shared(
            FailPlugin::new("fail-end", FailStage::End),
        )];

        let error: CompileError = parse(
            String::from("const a = 1;"),
            ParseOptions::default(),
            plugins,
        )
        .await
        .expect_err("an end failure fails the invocation");

        assert_eq!(error.message(), "fail-end");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_no_end_when_options_fixpoint_fails() {
        let ended: Arc<Mutex<Vec<Option<String>>>> =
            Arc::new(Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(EndObserverPlugin::new(&ended)),
            Plugin::new_shared(FailPlugin::new(
                "fail-options",
                FailStage::Options,
            )),
        ];

        let error: CompileError = compile(
            String::from("const a = 1;"),
            CompileOptions::default(),
            plugins,
        )
        .await
        .expect_err("options failure fails the invocation");

        assert_eq!(error.message(), "fail-options");

        assert!(
            ended.lock().unwrap().is_empty(),
            "end must not run when the options fixpoint fails"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_parse_error_preserves_source() {
        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(SourceFailParsePlugin)];

        let error: CompileError = parse(
            String::from("const x = 1;"),
            ParseOptions::default(),
            plugins,
        )
        .await
        .expect_err("the parse hook failure fails the invocation");

        assert_eq!(error.message(), "boom");

        let source: &ErrorSource =
            error.source().expect("the source is preserved");

        assert_eq!(source.file(), "input.ts");
        assert_eq!(source.code(), "const x = 1;");
        assert_eq!(source.span(), Span::new(6, 7));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_end_failure_source_replaces_the_stage_source() {
        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(SourceFailParsePlugin),
            Plugin::new_shared(SourceFailEndPlugin),
        ];

        let error: CompileError = parse(
            String::from("const x = 1;"),
            ParseOptions::default(),
            plugins,
        )
        .await
        .expect_err("both the stage and end hook fail");

        assert_eq!(error.message(), "boom");

        let rendered: String = error.to_string();

        assert!(!rendered.contains("end hook failed"), "{rendered}");

        let source: &ErrorSource =
            error.source().expect("the end failure source is kept");

        assert_eq!(source.file(), "end.ts");
        assert_eq!(source.code(), "end();");
        assert_eq!(source.span(), Span::new(0, 3));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_transform_error_preserves_source() {
        let ast: Ast = parse_fixture("const x = 1;").expect("fixture parses");

        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(SourceFailTransformPlugin)];

        let error: CompileError =
            rewrite(ast, RewriteOptions::default(), plugins)
                .await
                .expect_err("the transform hook failure fails the invocation");

        assert_eq!(error.message(), "boom");

        let source: &ErrorSource =
            error.source().expect("the source is preserved");

        assert_eq!(source.file(), "input.ts");
        assert_eq!(source.code(), "const x = 1;");
        assert_eq!(source.span(), Span::new(6, 7));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_parse_options_flow_into_start_resolved() {
        let seen: Arc<Mutex<Vec<ResolvedOptions>>> =
            Arc::new(Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(StartObserverPlugin::new(&seen))];

        parse(String::from("const a = 1;"), ParseOptions::default(), plugins)
            .await
            .expect("parse succeeds");

        let options: Vec<ResolvedOptions> = seen.lock().unwrap().clone();

        assert_eq!(options.len(), 1);

        assert_eq!(options[0].file, "index.ts");
        assert_eq!(options[0].language, Language::TS);
        assert_eq!(options[0].source_type, SourceType::Unambiguous);
        assert_eq!(options[0].plugins, vec![String::from("start-observer")]);
    }
}
