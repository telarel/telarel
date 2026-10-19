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
    HookUsage, OutputSourceMap, OutputSourceMapMode, Parser, ParserOptions,
    ResolvableOptions, ResolvedOptions, apply_output_source_map_mode,
    resolve_input_source_map, resolve_options,
};
use telarel_plugin::__internal::PluginDriver;
use telarel_plugin::{
    CommonPluginContext, EndArgs, FinalizeArgs, ModuleInfo, OptionsArgs,
    PluginContext, PrepareArgs, SharedPluginable, StartArgs, TransformArgs,
    TransformOutput, options_fixpoint,
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

/// Whether re-running the builtin reproduces `error` exactly.
///
/// The driver wraps a plugin hook error with a ``` `{name}` parse ``` /
/// ``` `{name}` generate ``` context, so a plugin error always renders with a
/// leading `` ` ``; a builtin error is either source-bound (its graphical
/// report starts with whitespace) or a source-less map message that starts
/// with a word, never a backtick. The renderings can therefore never coincide,
/// so re-running the builtin and comparing strings never misattributes a
/// plugin error — no driver-side origin signal is needed.
fn builtin_reproduces_error(
    result: Result<(), CompileError>,
    error: &CompileError,
) -> bool {
    match result {
        | Ok(()) => false,
        | Err(builtin_error) => builtin_error.to_string() == error.to_string(),
    }
}

/// Wrap a `parse` stage error: a plugin-hook error gains the `parse hook: `
/// prefix, a builtin error passes through raw.
fn wrap_parse_error(
    driver: &PluginDriver,
    error: CompileError,
    builtin: &Parser,
) -> CompileError {
    if !driver.usage().contains(HookUsage::Parse) {
        return error;
    }

    let reproduced: bool = builtin_reproduces_error(
        builtin.clone().parse().map(|_result: AstResult| ()),
        &error,
    );

    if reproduced {
        return error;
    }

    CompileError::from_message(&format!("parse hook: {error}"))
}

fn wrap_generate_error(
    driver: &PluginDriver,
    error: CompileError,
    builtin: &Generator<'_>,
) -> CompileError {
    if !driver.usage().contains(HookUsage::Generate) {
        return error;
    }

    let reproduced: bool = builtin_reproduces_error(
        builtin.clone().generate().map(|_result: CodeResult| ()),
        &error,
    );

    if reproduced {
        return error;
    }

    CompileError::from_message(&format!("generate hook: {error}"))
}

/// The resolved output mode; `None` (and `Options` defaults) fall back to
/// `External`.
fn output_mode(output: Option<&OutputSourceMap>) -> OutputSourceMapMode {
    match output {
        | Some(OutputSourceMap::Mode(mode)) => *mode,
        | Some(OutputSourceMap::Options(options)) => options.mode,
        | None => OutputSourceMapMode::default(),
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

    driver.parse(ctx, code, &options, &builtin).await.map_err(
        |error: CompileError| wrap_parse_error(driver, error, &builtin),
    )
}

async fn rewrite_stages(
    driver: &PluginDriver,
    ctx: &PluginContext<'_>,
    ast: Ast,
) -> Result<Ast, CompileError> {
    driver.prepare(ctx, PrepareArgs { ast: &ast }).await.map_err(
        |error: anyhow::Error| {
            CompileError::from_message(&format!("prepare hook: {error:#}"))
        },
    )?;

    let transformed: Option<TransformOutput> = driver
        .transform(ctx, TransformArgs { ast: ast.clone() })
        .await
        .map_err(|error: anyhow::Error| {
            CompileError::from_message(&format!("transform hook: {error:#}"))
        })?;

    let ast: Ast = match transformed {
        | Some(output) => output.ast,
        | None => ast,
    };

    driver.finalize(ctx, FinalizeArgs { ast: &ast }).await.map_err(
        |error: anyhow::Error| {
            CompileError::from_message(&format!("finalize hook: {error:#}"))
        },
    )?;

    Ok(ast)
}

/// The `generate` stage: the builtin generator is the terminal fallback after
/// every `generate` hook defers. The resolved output mode is applied uniformly,
/// regardless of which hook won.
async fn generate_stage(
    driver: &PluginDriver,
    ctx: &PluginContext<'_>,
    resolved: &ResolvedOptions,
    ast: &Ast,
) -> Result<CodeResult, CompileError> {
    let options: GeneratorOptions = GeneratorOptions::from(resolved);

    let builtin: Generator<'_> = Generator::new(ast, options.clone());

    let result: CodeResult =
        driver.generate(ctx, ast, &options, &builtin).await.map_err(
            |error: CompileError| wrap_generate_error(driver, error, &builtin),
        )?;

    Ok(apply_output_source_map_mode(
        result,
        output_mode(resolved.source_map.output.as_ref()),
    ))
}

/// Run `end` around a stage result.
///
/// On success `error` is unset; on a stage failure it carries the first stage
/// error and the ORIGINAL error wins, with an `end` failure appended (never
/// swallowed). `end` itself runs on every path after the settle.
async fn finish<T>(
    driver: &PluginDriver,
    ctx: &PluginContext<'_>,
    result: Result<T, CompileError>,
) -> Result<T, CompileError> {
    match result {
        | Ok(output) => {
            driver.end(ctx, &EndArgs { error: None }).await.map_err(
                |error: anyhow::Error| {
                    CompileError::from_message(&format!("end hook: {error:#}"))
                },
            )?;

            Ok(output)
        },
        | Err(error) => {
            let mut args: EndArgs = EndArgs { error: Some(error) };

            match driver.end(ctx, &args).await {
                | Ok(()) => Err(args.error.take().expect("end error present")),
                | Err(end_error) => {
                    let original: CompileError =
                        args.error.take().expect("end error present");

                    Err(CompileError::from_message(&format!(
                        "{original} (end hook failed: {end_error:#})"
                    )))
                },
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
        options_fixpoint(&common, &args).await.map_err(
            |error: anyhow::Error| {
                CompileError::from_message(&format!("options hook: {error:#}"))
            },
        )?;

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
        .map_err(|error: anyhow::Error| {
            CompileError::from_message(&format!("start hook: {error:#}"))
        }) {
        | Ok(()) => stage.run(&driver, &ctx, &resolved).await,
        | Err(error) => Err(error),
    };

    finish(&driver, &ctx, result).await
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::Mutex;

    use telarel_common::{
        CompileError, CompileOptions, Language, ParseOptions, ResolvedOptions,
        SourceType,
    };
    use telarel_plugin::{Plugin, SharedPluginable};

    use crate::common::test_util::{
        EndObserverPlugin, FailPlugin, FailStage, StartObserverPlugin,
    };
    use crate::functions::compile::compile;
    use crate::functions::parse::parse;

    #[tokio::test(flavor = "current_thread")]
    async fn test_end_failure_is_appended_to_the_stage_error() {
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

        let rendered: String = error.to_string();

        assert!(rendered.contains("start hook:"), "{rendered}");
        assert!(rendered.contains(" (end hook failed: "), "{rendered}");
        assert!(rendered.contains("`fail-end` end"), "{rendered}");
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

        let rendered: String = error.to_string();

        assert!(rendered.starts_with("end hook: "), "{rendered}");
        assert!(rendered.contains("`fail-end` end"), "{rendered}");
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

        assert!(error.to_string().contains("options hook:"), "{error}");

        assert!(
            ended.lock().unwrap().is_empty(),
            "end must not run when the options fixpoint fails"
        );
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
