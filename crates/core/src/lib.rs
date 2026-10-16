//! # Telarel Core
//!
//! A pipeline orchestrator for the compiler.
//!
//! This crate drives the per-file compile pipeline: it runs plugin hooks,
//! parses the source, applies transforms, and produces code with a map.

use telarel_common::{
    Ast, AstResult, CodeResult, CompileError, Generator, GeneratorOptions,
    HookUsage, Language, Parser, ParserOptions, ResolvableOptions,
    ResolvedOptions, SourceMapOptions, SourceType, infer_language,
    resolve_source_type,
};
use telarel_plugin::__internal::PluginDriver;
use telarel_plugin::{
    CommonPluginContext, EndArgs, FinalizeArgs, ModuleInfo, OptionsArgs,
    PluginContext, PrepareArgs, SharedPluginable, StartArgs, TransformArgs,
    TransformOutput, options_fixpoint,
};

pub use telarel_common::CompileOptions;
pub use telarel_common::SourceMap;

/// Result of a successful compile.
#[derive(Debug, Clone)]
pub struct CompileOutput {
    /// Generated code.
    pub code: String,
    /// Source map, when a stage produced one.
    pub map: Option<SourceMap>,
}

/// Resolve an omitted `cwd` to the runtime's process working directory,
/// falling back to `/` when it cannot be read.
fn resolve_process_cwd() -> String {
    std::env::current_dir()
        .map(|p: std::path::PathBuf| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| String::from("/"))
}

/// Whether re-running the builtin reproduces `error`.
///
/// The driver returns a single [`CompileError`] for both a plugin hook error
/// and a builtin error. A plugin error is message-based (it carries the
/// `` `{name}` parse ``/`` `{name}` generate `` context), while a builtin error
/// is source-bound; re-running the builtin reproduces the builtin error
/// exactly, so equal renderings identify it.
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

/// Wrap a `generate` stage error; see [`wrap_parse_error`] for the
/// plugin-vs-builtin distinction.
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

/// Run the settled pipeline stages: `start`, the always-parsed `parse`,
/// `prepare`/`transform`/`finalize` over the AST, and `generate`.
///
/// The `end` hook is not run here; the caller runs it once, on every path.
async fn run_pipeline(
    driver: &PluginDriver,
    plugin_ctx: &PluginContext<'_>,
    resolved_options: &ResolvedOptions,
) -> Result<CompileOutput, CompileError> {
    driver
        .start(plugin_ctx, &StartArgs { options: resolved_options.clone() })
        .await
        .map_err(|error| {
            CompileError::from_message(&format!("start hook: {error:#}"))
        })?;

    // Parse ALWAYS: there is no more parse-skip gate. The builtin parser is
    // the terminal fallback after every `parse` hook defers.
    let parser_options: ParserOptions = ParserOptions {
        cwd: resolved_options.cwd.clone(),
        file: resolved_options.file.clone(),
        language: resolved_options.language,
        source_type: resolved_options.source_type,
        source_map: SourceMapOptions::default(),
    };

    let builtin_parser: Parser =
        Parser::new(resolved_options.code.clone(), parser_options.clone());

    let ast: Ast = driver
        .parse(
            plugin_ctx,
            &resolved_options.code,
            &parser_options,
            &builtin_parser,
        )
        .await
        .map_err(|error| wrap_parse_error(driver, error, &builtin_parser))?
        .ast;

    driver.prepare(plugin_ctx, PrepareArgs { ast: &ast }).await.map_err(
        |error| CompileError::from_message(&format!("prepare hook: {error:#}")),
    )?;

    let transform_output: Option<TransformOutput> = driver
        .transform(plugin_ctx, TransformArgs { ast: &ast })
        .await
        .map_err(|error| {
            CompileError::from_message(&format!("transform hook: {error:#}"))
        })?;

    let ast: Ast = match transform_output {
        | Some(output) => output.ast,
        | None => ast,
    };

    driver.finalize(plugin_ctx, FinalizeArgs { ast: &ast }).await.map_err(
        |error| {
            CompileError::from_message(&format!("finalize hook: {error:#}"))
        },
    )?;

    let generator_options: GeneratorOptions = GeneratorOptions {
        cwd: resolved_options.cwd.clone(),
        file: resolved_options.file.clone(),
        source_map: SourceMapOptions::default(),
    };

    let builtin_generator: Generator<'_> =
        Generator::new(&ast, generator_options.clone());

    let code_result: CodeResult = driver
        .generate(plugin_ctx, &ast, &generator_options, &builtin_generator)
        .await
        .map_err(|error| {
            wrap_generate_error(driver, error, &builtin_generator)
        })?;

    Ok(CompileOutput { code: code_result.code, map: code_result.map })
}

/// Run the per-file pipeline over `options` with `plugins`.
pub async fn compile(
    options: CompileOptions,
    plugins: Vec<SharedPluginable>,
) -> Result<CompileOutput, CompileError> {
    // The old options shape still carries `code`; the fixpoint payload does
    // not, so split it out before building the resolvable options.
    let code: String = options.code.clone();

    let resolvable: ResolvableOptions = ResolvableOptions {
        cwd: options.cwd.clone(),
        file: Some(options.file.clone()),
        language: options.language,
        source_type: options.source_type,
        source_map: None,
    };

    // The common context is constructed ONCE per compile: the `options`
    // fixpoint runs against this instance.
    let common: CommonPluginContext = CommonPluginContext::default();

    let args: OptionsArgs = OptionsArgs { options: resolvable, plugins };

    // The `options` hooks run as a fixpoint BEFORE any driver exists; plugins
    // injected into the args join the settled list.
    let (args, settled_plugins): (OptionsArgs, Vec<SharedPluginable>) =
        options_fixpoint(&common, &args).await.map_err(|error| {
            CompileError::from_message(&format!("options hook: {error:#}"))
        })?;

    // A late options hook may omit `cwd`/`file`; re-resolve to the documented
    // defaults before they reach `start` and later hooks.
    let cwd: String =
        args.options.cwd.clone().unwrap_or_else(resolve_process_cwd);

    let file: String =
        args.options.file.clone().unwrap_or_else(|| String::from("index.js"));

    let language: Language = infer_language(&file, args.options.language);

    let source_type: SourceType = resolve_source_type(
        &file,
        args.options.language,
        args.options.source_type,
    );

    // The driver is built from the SETTLED list: its usage aggregate covers
    // plugins injected during the `options` stage.
    let driver: PluginDriver = PluginDriver::new(settled_plugins);

    let resolved_options: ResolvedOptions = ResolvedOptions {
        cwd: cwd.clone(),
        file: file.clone(),
        code: code.clone(),
        language,
        source_type,
        plugins: driver.settled_names().to_vec(),
    };

    let module: ModuleInfo<'_> =
        ModuleInfo { file: &file, language, source_type };

    let plugin_ctx: PluginContext<'_> = PluginContext::new(&cwd, module);

    let result: Result<CompileOutput, CompileError> =
        run_pipeline(&driver, &plugin_ctx, &resolved_options).await;

    // `end` runs on EVERY path after the settle. On success `error` is unset;
    // on a stage failure it carries the first stage error and the ORIGINAL
    // error wins, with an `end` failure appended (never swallowed). If the
    // options fixpoint fails, no driver exists, so `end` does not run.
    match result {
        | Ok(output) => {
            driver.end(&plugin_ctx, &EndArgs { error: None }).await.map_err(
                |error| {
                    CompileError::from_message(&format!("end hook: {error:#}"))
                },
            )?;

            Ok(output)
        },
        | Err(error) => {
            let mut args: EndArgs = EndArgs { error: Some(error) };

            match driver.end(&plugin_ctx, &args).await {
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

// The house style for the test hook impls is the explicit RPITIT form
// (never `async fn`), so the `manual_async_fn` lint is suppressed here.
#[cfg(test)]
#[allow(clippy::manual_async_fn)]
mod tests {
    use std::borrow::Cow;
    use std::sync::Arc;
    use std::sync::Mutex;

    use telarel_common::{
        Ast, AstResult, CodeResult, CompileOptions, Language, ParseAstOptions,
        ResolvedOptions, SourceMap, SourceType, parse_ast,
    };
    use telarel_plugin::{
        CommonPluginContext, EndArgs, FinalizeArgs, GenerateArgs,
        GenerateReturn, HookUsage, NotifyReturn, OptionsArgs, OptionsReturn,
        ParseArgs, ParseReturn, Plugin, PluginContext, PrepareArgs,
        SharedPluginable, StartArgs, TransformArgs, TransformReturn,
    };

    use oxc::allocator::Allocator;
    use oxc::ast::ast::{
        Directive, IdentifierReference, Program, StringLiteral,
    };
    use oxc::ast::builder::AstBuilder;
    use oxc::ast_visit::VisitMut;
    use oxc::ast_visit::walk_mut;
    use oxc::span::SPAN;
    use oxc::str::Ident;

    use super::*;

    fn options() -> CompileOptions {
        CompileOptions {
            file: "index.ts".to_string(),
            code: "const a = 1;".to_string(),
            ..Default::default()
        }
    }

    fn code_options(code: &str) -> CompileOptions {
        CompileOptions {
            file: "index.ts".to_string(),
            code: code.to_string(),
            ..Default::default()
        }
    }

    fn parse_fixture(code: &str) -> anyhow::Result<Ast> {
        parse_ast(
            code.to_string(),
            ParseAstOptions {
                cwd: String::from("/repo"),
                file: String::from("index.ts"),
                language: None,
                source_type: None,
                input_map: None,
            },
        )
        .map_err(anyhow::Error::from)
    }

    #[derive(Debug)]
    struct ObserveOptionsPlugin {
        observed: Arc<Mutex<Vec<Option<String>>>>,
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
            let cwd: Option<String> = args.options.cwd.clone();

            async move {
                self.observed.lock().unwrap().push(cwd);

                Ok(None)
            }
        }
    }

    #[derive(Debug)]
    struct RewriteFileOptionsPlugin;

    impl Plugin for RewriteFileOptionsPlugin {
        fn name(&self) -> Cow<'static, str> {
            "rewrite-file-options".into()
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

                next.options.file = Some("rewritten.ts".to_string());

                Ok(Some(next))
            }
        }
    }

    #[derive(Debug)]
    struct DropCwdOptionsPlugin;

    impl Plugin for DropCwdOptionsPlugin {
        fn name(&self) -> Cow<'static, str> {
            "drop-cwd-options".into()
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

                next.options.cwd = None;

                Ok(Some(next))
            }
        }
    }

    #[derive(Debug)]
    struct DropFileOptionsPlugin;

    impl Plugin for DropFileOptionsPlugin {
        fn name(&self) -> Cow<'static, str> {
            "drop-file-options".into()
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

                next.options.file = None;

                Ok(Some(next))
            }
        }
    }

    #[derive(Debug)]
    struct FailingOptionsPlugin;

    impl Plugin for FailingOptionsPlugin {
        fn name(&self) -> Cow<'static, str> {
            "failing-options".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Options
        }

        fn options<'a>(
            &'a self,
            _ctx: &'a CommonPluginContext,
            _args: &'a OptionsArgs,
        ) -> impl Future<Output = OptionsReturn> + Send {
            async { Err(anyhow::anyhow!("stage boom")) }
        }
    }

    // Injects a plugin into the args' plugin list ONCE (first invocation):
    // the fixpoint must run the injected plugin's hooks in the next pass.
    // Injecting on every invocation would be a plugin cycle, correctly
    // aborted by the depth guard.
    #[derive(Debug)]
    struct InjectOptionsPlugin {
        injected: SharedPluginable,
        remaining: Mutex<usize>,
    }

    impl Plugin for InjectOptionsPlugin {
        fn name(&self) -> Cow<'static, str> {
            "inject-options".into()
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
                {
                    let mut remaining: std::sync::MutexGuard<'_, usize> =
                        self.remaining.lock().unwrap();

                    if *remaining == 0 {
                        return Ok(None);
                    }

                    *remaining -= 1;
                }

                let mut next: OptionsArgs = args.clone();

                next.plugins.push(self.injected.clone());

                Ok(Some(next))
            }
        }
    }

    #[derive(Debug)]
    struct ObserveStartPlugin {
        seen: Arc<Mutex<Vec<ResolvedOptions>>>,
    }

    impl Plugin for ObserveStartPlugin {
        fn name(&self) -> Cow<'static, str> {
            "observe-start".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Start
        }

        fn start<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            args: &'a StartArgs,
        ) -> impl Future<Output = NotifyReturn> + Send {
            let options: ResolvedOptions = args.options.clone();

            async move {
                self.seen.lock().unwrap().push(options);

                Ok(())
            }
        }
    }

    #[derive(Debug)]
    struct ObserveStartCwdPlugin {
        observed: Arc<Mutex<Vec<String>>>,
    }

    impl Plugin for ObserveStartCwdPlugin {
        fn name(&self) -> Cow<'static, str> {
            "observe-start-cwd".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Start
        }

        fn start<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            args: &'a StartArgs,
        ) -> impl Future<Output = NotifyReturn> + Send {
            let cwd: String = args.options.cwd.clone();

            async move {
                self.observed.lock().unwrap().push(cwd);

                Ok(())
            }
        }
    }

    #[derive(Debug)]
    struct StartObserverPlugin {
        name: &'static str,
        observer: Arc<Mutex<Vec<ResolvedOptions>>>,
    }

    impl Plugin for StartObserverPlugin {
        fn name(&self) -> Cow<'static, str> {
            self.name.into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Start
        }

        fn start<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            args: &'a StartArgs,
        ) -> impl Future<Output = NotifyReturn> + Send {
            let options: ResolvedOptions = args.options.clone();
            let observer: Arc<Mutex<Vec<ResolvedOptions>>> =
                Arc::clone(&self.observer);

            async move {
                observer.lock().unwrap().push(options);

                Ok(())
            }
        }
    }

    #[derive(Debug)]
    struct FailingStartPlugin;

    impl Plugin for FailingStartPlugin {
        fn name(&self) -> Cow<'static, str> {
            "failing-start".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Start
        }

        fn start<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            _args: &'a StartArgs,
        ) -> impl Future<Output = NotifyReturn> + Send {
            async { Err(anyhow::anyhow!("start boom")) }
        }
    }

    #[derive(Debug)]
    struct EndObserverPlugin {
        seen: Arc<Mutex<Vec<Option<String>>>>,
    }

    impl Plugin for EndObserverPlugin {
        fn name(&self) -> Cow<'static, str> {
            "observe-end".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::End
        }

        fn end<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            args: &'a EndArgs,
        ) -> impl Future<Output = NotifyReturn> + Send {
            let rendered: Option<String> =
                args.error.as_ref().map(ToString::to_string);

            async move {
                self.seen.lock().unwrap().push(rendered);

                Ok(())
            }
        }
    }

    #[derive(Debug)]
    struct FailingEndPlugin;

    impl Plugin for FailingEndPlugin {
        fn name(&self) -> Cow<'static, str> {
            "failing-end".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::End
        }

        fn end<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            _args: &'a EndArgs,
        ) -> impl Future<Output = NotifyReturn> + Send {
            async { Err(anyhow::anyhow!("end boom")) }
        }
    }

    #[derive(Debug)]
    struct NoopPlugin;

    impl Plugin for NoopPlugin {
        fn name(&self) -> Cow<'static, str> {
            "noop".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Prepare
        }
    }

    #[derive(Debug)]
    struct ObservePreparePlugin {
        seen: Arc<Mutex<Vec<String>>>,
    }

    impl Plugin for ObservePreparePlugin {
        fn name(&self) -> Cow<'static, str> {
            "observe-prepare".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Prepare
        }

        fn prepare<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            args: PrepareArgs<'a>,
        ) -> impl Future<Output = NotifyReturn> + Send + 'a {
            let source: String = args.ast.source().to_string();

            async move {
                self.seen.lock().unwrap().push(source);

                Ok(())
            }
        }
    }

    #[derive(Debug)]
    struct ObserveFinalizePlugin {
        seen: Arc<Mutex<Vec<String>>>,
    }

    impl Plugin for ObserveFinalizePlugin {
        fn name(&self) -> Cow<'static, str> {
            "observe-finalize".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Finalize
        }

        fn finalize<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            args: FinalizeArgs<'a>,
        ) -> impl Future<Output = NotifyReturn> + Send + 'a {
            let source: String = args.ast.source().to_string();

            async move {
                self.seen.lock().unwrap().push(source);

                Ok(())
            }
        }
    }

    /// Records the observed AST program address in `prepare` and `finalize`.
    #[derive(Debug)]
    struct ObserveAstAddressPlugin {
        usage: HookUsage,
        seen: Arc<Mutex<Vec<usize>>>,
    }

    impl Plugin for ObserveAstAddressPlugin {
        fn name(&self) -> Cow<'static, str> {
            "observe-ast-address".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            self.usage
        }

        fn prepare<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            args: PrepareArgs<'a>,
        ) -> impl Future<Output = NotifyReturn> + Send + 'a {
            let address: usize =
                std::ptr::from_ref(args.ast.program()) as usize;

            async move {
                self.seen.lock().unwrap().push(address);

                Ok(())
            }
        }

        fn finalize<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            args: FinalizeArgs<'a>,
        ) -> impl Future<Output = NotifyReturn> + Send + 'a {
            let address: usize =
                std::ptr::from_ref(args.ast.program()) as usize;

            async move {
                self.seen.lock().unwrap().push(address);

                Ok(())
            }
        }
    }

    #[derive(Debug)]
    struct FailingPreparePlugin;

    impl Plugin for FailingPreparePlugin {
        fn name(&self) -> Cow<'static, str> {
            "failing-prepare".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Prepare
        }

        fn prepare<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            _args: PrepareArgs<'a>,
        ) -> impl Future<Output = NotifyReturn> + Send + 'a {
            async { Err(anyhow::anyhow!("stage boom")) }
        }
    }

    #[derive(Debug)]
    struct FailingFinalizePlugin;

    impl Plugin for FailingFinalizePlugin {
        fn name(&self) -> Cow<'static, str> {
            "failing-finalize".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Finalize
        }

        fn finalize<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            _args: FinalizeArgs<'a>,
        ) -> impl Future<Output = NotifyReturn> + Send + 'a {
            async { Err(anyhow::anyhow!("stage boom")) }
        }
    }

    #[derive(Debug)]
    struct FailingParsePlugin;

    impl Plugin for FailingParsePlugin {
        fn name(&self) -> Cow<'static, str> {
            "fail-parse".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Parse
        }

        fn parse<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            _args: ParseArgs<'a>,
        ) -> impl Future<Output = ParseReturn> + Send + 'a {
            async { Err(anyhow::anyhow!("parse boom")) }
        }
    }

    #[derive(Debug)]
    struct WinningParsePlugin;

    impl Plugin for WinningParsePlugin {
        fn name(&self) -> Cow<'static, str> {
            "winning-parse".into()
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
                let ast: Ast = parse_fixture("let custom = 1;")?;

                Ok(Some(AstResult { ast }))
            }
        }
    }

    #[derive(Debug)]
    struct FailingGeneratePlugin;

    impl Plugin for FailingGeneratePlugin {
        fn name(&self) -> Cow<'static, str> {
            "fail-generate".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Generate
        }

        fn generate<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            _args: GenerateArgs<'a>,
        ) -> impl Future<Output = GenerateReturn> + Send + 'a {
            async { Err(anyhow::anyhow!("generate boom")) }
        }
    }

    /// A winning `generate` hook that returns no map.
    #[derive(Debug)]
    struct MaplessGeneratePlugin;

    impl Plugin for MaplessGeneratePlugin {
        fn name(&self) -> Cow<'static, str> {
            "mapless-generate".into()
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
                    code: String::from("custom();"),
                    map: None,
                }))
            }
        }
    }

    const TARGET: &str = "console";
    const REPLACEMENT: &str = "consolex";
    const DIRECTIVE: &str = "x-telarel-chain";
    const REPLACED_CODE: &str = "console.log(2);";

    #[derive(Debug)]
    struct ReplaceRootWithConsolePlugin;

    impl Plugin for ReplaceRootWithConsolePlugin {
        fn name(&self) -> Cow<'static, str> {
            "replace-root-with-console".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Transform
        }

        fn transform<'a>(
            &'a self,
            ctx: &'a PluginContext<'_>,
            _args: TransformArgs<'a>,
        ) -> impl Future<Output = TransformReturn> + Send + 'a {
            async move {
                let ast: Ast = parse_ast(
                    REPLACED_CODE.to_string(),
                    ParseAstOptions {
                        cwd: String::from("/repo"),
                        file: ctx.module.file.to_string(),
                        language: None,
                        source_type: None,
                        input_map: None,
                    },
                )?;

                Ok(Some(TransformOutput { ast }))
            }
        }
    }

    struct Renamer<'x> {
        allocator: &'x Allocator,
    }

    impl<'x> VisitMut<'x> for Renamer<'x> {
        fn visit_identifier_reference(
            &mut self,
            ident: &mut IdentifierReference<'x>,
        ) {
            if ident.name.as_str() == TARGET {
                ident.name = Ident::from_str_in(REPLACEMENT, &self.allocator);
            }
        }
    }

    #[derive(Debug)]
    struct RenameCalleePlugin;

    impl Plugin for RenameCalleePlugin {
        fn name(&self) -> Cow<'static, str> {
            "rename-callee".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Transform
        }

        fn transform<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            args: TransformArgs<'a>,
        ) -> impl Future<Output = TransformReturn> + Send + 'a {
            async move {
                let mut ast: Ast = args.ast.clone();

                ast.with_mut(
                    |allocator: &Allocator, program: &mut Program<'_>| {
                        let mut renamer: Renamer<'_> = Renamer { allocator };

                        walk_mut::walk_program(&mut renamer, program);
                    },
                );

                Ok(Some(TransformOutput { ast }))
            }
        }
    }

    #[derive(Debug)]
    struct AppendDirectivePlugin;

    impl Plugin for AppendDirectivePlugin {
        fn name(&self) -> Cow<'static, str> {
            "append-directive".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Transform
        }

        fn transform<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            args: TransformArgs<'a>,
        ) -> impl Future<Output = TransformReturn> + Send + 'a {
            async move {
                let mut ast: Ast = args.ast.clone();

                ast.with_mut(
                    |allocator: &Allocator, program: &mut Program<'_>| {
                        let builder: AstBuilder<'_> =
                            AstBuilder::new(allocator);

                        let string_literal: StringLiteral<'_> =
                            StringLiteral::new(SPAN, DIRECTIVE, None, &builder);

                        let directive: Directive<'_> = Directive::new(
                            SPAN,
                            string_literal,
                            DIRECTIVE,
                            &builder,
                        );

                        program.directives.push(directive);
                    },
                );

                Ok(Some(TransformOutput { ast }))
            }
        }
    }

    #[derive(Debug)]
    struct NoopTransformPlugin;

    impl Plugin for NoopTransformPlugin {
        fn name(&self) -> Cow<'static, str> {
            "noop-transform".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Transform
        }

        fn transform<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            _args: TransformArgs<'a>,
        ) -> impl Future<Output = TransformReturn> + Send + 'a {
            async { Ok(None) }
        }
    }

    #[derive(Debug)]
    struct FailingTransformHookPlugin;

    impl Plugin for FailingTransformHookPlugin {
        fn name(&self) -> Cow<'static, str> {
            "failing-transform-hook".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Transform
        }

        fn transform<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            _args: TransformArgs<'a>,
        ) -> impl Future<Output = TransformReturn> + Send + 'a {
            async { Err(anyhow::anyhow!("transform boom")) }
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_no_plugins_codegens_original() {
        let out: CompileOutput = compile(options(), vec![]).await.unwrap();

        assert_eq!(out.code, "const a = 1;");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_no_plugins_produces_real_codegen_map() {
        // Parse always runs now, so the map is the real codegen map (never an
        // identity map).
        let out: CompileOutput = compile(options(), vec![]).await.unwrap();

        let map: SourceMap = out.map.expect("codegen produces a real map");

        assert_eq!(map.get_source(0), Some("index.ts"));

        assert!(
            map.get_tokens().next().is_some(),
            "mappings must be non-empty"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_invalid_code_errors_without_plugins() {
        // No plugin declares transform, yet parse still runs: invalid syntax
        // errors instead of passing through verbatim.
        let error: CompileError =
            compile(code_options("const = ;"), vec![]).await.unwrap_err();

        let message: String = error.to_string();

        assert!(message.contains("index.ts"), "{}", message);

        // The builtin parse error passes through raw (no stage prefix).
        assert!(!message.contains("parse hook:"), "{}", message);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_invalid_code_errors_with_transform_plugin() {
        let error: CompileError = compile(
            code_options("const = ;"),
            vec![Plugin::new_shared(RenameCalleePlugin)],
        )
        .await
        .unwrap_err();

        assert!(error.to_string().contains("index.ts"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_noop_prepare_plugin_still_parses() {
        // A plugin declaring only PRE does not force the parse path anymore:
        // parse always runs, invalid code fails, and valid code is codegen'd.
        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(NoopPlugin)];

        let out: CompileOutput =
            compile(code_options("console.log(1);"), plugins).await.unwrap();

        assert_eq!(out.code, "console.log(1);");

        assert!(out.map.is_some(), "real codegen map");

        let error: CompileError =
            compile(code_options("const = ;"), vec![]).await.unwrap_err();

        assert!(error.to_string().contains("index.ts"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_codegen_strips_trailing_newline() {
        let out: CompileOutput =
            compile(code_options("const a = 1;\n"), vec![]).await.unwrap();

        assert_eq!(out.code, "const a = 1;");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_codegen_strips_crlf_trailing_newline() {
        let out: CompileOutput =
            compile(code_options("const a = 1;\r\n"), vec![]).await.unwrap();

        assert_eq!(out.code, "const a = 1;");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_codegen_preserves_interior_statements() {
        // Codegen keeps one statement per line; interior newlines survive.
        let out: CompileOutput =
            compile(code_options("const a = 1;\nconst b = 2;"), vec![])
                .await
                .unwrap();

        assert_eq!(out.code, "const a = 1;\nconst b = 2;");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_codegen_strips_lone_cr_trailing_newline() {
        let out: CompileOutput =
            compile(code_options("const a = 1;\r"), vec![]).await.unwrap();

        assert_eq!(out.code, "const a = 1;");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_output_map_is_owned_alias() {
        // `SourceMap` in `CompileOutput` must be the owned-map alias: assign it
        // to an explicitly-aliased variable to prove the types agree.
        let out: CompileOutput = compile(options(), vec![]).await.unwrap();

        let map: SourceMap = out.map.expect("codegen map");

        assert_eq!(map.get_source(0), Some("index.ts"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_transform_plugin_replaces() {
        let out = compile(
            code_options("console.log(1);"),
            vec![Plugin::new_shared(RenameCalleePlugin)],
        )
        .await
        .unwrap();

        assert!(out.code.contains("consolex"), "{}", out.code);

        let map: SourceMap = out.map.expect("codegen map");

        assert!(
            map.get_tokens().next().is_some(),
            "mappings must be non-empty"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_transform_sourcemap_points_at_original() {
        let out = compile(
            code_options("console.log(1);"),
            vec![Plugin::new_shared(RenameCalleePlugin)],
        )
        .await
        .unwrap();

        let map: SourceMap = out.map.expect("codegen map");

        assert_eq!(map.get_source(0), Some("index.ts"));

        assert!(
            map.get_tokens().next().is_some(),
            "mappings must be non-empty"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_transform_chain_multiple_replacements() {
        // Two replacing plugins in sequence: the rename must survive into the
        // second plugin and both transformations must appear in the final
        // code (proves the chain carries each replacement by reference).
        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(RenameCalleePlugin),
            Plugin::new_shared(AppendDirectivePlugin),
        ];

        let out =
            compile(code_options("console.log(1);"), plugins).await.unwrap();

        assert!(out.code.contains("consolex"), "{}", out.code);

        assert!(out.code.contains("x-telarel-chain"), "{}", out.code);

        assert_eq!(out.map.expect("map").get_source(0), Some("index.ts"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_transform_some_then_second_replacement_kept() {
        // Some(replace root) followed by a second replacing plugin: the final
        // codegen must contain BOTH replacements (the second plugin receives
        // the first's returned program).
        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(ReplaceRootWithConsolePlugin),
            Plugin::new_shared(RenameCalleePlugin),
        ];

        let out: CompileOutput =
            compile(code_options("console.log(1);"), plugins).await.unwrap();

        assert!(out.code.contains("consolex.log(2)"), "{}", out.code);

        assert_eq!(out.map.expect("map").get_source(0), Some("index.ts"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_transform_noop_plugin_keeps_original() {
        let out = compile(
            code_options("console.log(1);"),
            vec![Plugin::new_shared(NoopTransformPlugin)],
        )
        .await
        .unwrap();

        assert!(out.code.contains("console.log(1);"), "{}", out.code);

        assert_eq!(out.map.expect("map").get_source(0), Some("index.ts"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_transform_mixed_chain_noop_then_replace() {
        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(NoopTransformPlugin),
            Plugin::new_shared(RenameCalleePlugin),
            Plugin::new_shared(NoopTransformPlugin),
        ];

        let out =
            compile(code_options("console.log(1);"), plugins).await.unwrap();

        assert!(out.code.contains("consolex"), "{}", out.code);

        assert_eq!(out.map.expect("map").get_source(0), Some("index.ts"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_transform_error_aborts_with_stage_prefix() {
        let error: CompileError = compile(
            options(),
            vec![Plugin::new_shared(FailingTransformHookPlugin)],
        )
        .await
        .unwrap_err();

        let message: String = error.to_string();

        assert!(message.contains("transform hook:"), "{}", message);
        assert!(message.contains("transform boom"), "{}", message);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_prepare_hook_error_aborts() {
        let error: CompileError =
            compile(options(), vec![Plugin::new_shared(FailingPreparePlugin)])
                .await
                .unwrap_err();

        let message: String = error.to_string();

        assert!(message.contains("prepare hook"), "{}", message);
        assert!(message.contains("stage boom"), "{}", message);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_finalize_hook_error_aborts() {
        let error: CompileError =
            compile(options(), vec![Plugin::new_shared(FailingFinalizePlugin)])
                .await
                .unwrap_err();

        let message: String = error.to_string();

        assert!(message.contains("finalize hook"), "{}", message);
        assert!(message.contains("stage boom"), "{}", message);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_start_error_aborts() {
        let error: CompileError =
            compile(options(), vec![Plugin::new_shared(FailingStartPlugin)])
                .await
                .unwrap_err();

        let message: String = error.to_string();

        assert!(message.contains("start hook"), "{}", message);
        assert!(message.contains("start boom"), "{}", message);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_parse_hook_error_wraps_with_stage_prefix() {
        let error: CompileError =
            compile(options(), vec![Plugin::new_shared(FailingParsePlugin)])
                .await
                .unwrap_err();

        let message: String = error.to_string();

        assert!(
            message.contains("parse hook: `fail-parse` parse"),
            "{}",
            message
        );

        assert!(message.contains("parse boom"), "{}", message);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_parse_hook_winning_ast_is_codegenned() {
        // A winning parse hook replaces the builtin parse; the builtin
        // generator then codegens the plugin's AST.
        let out: CompileOutput =
            compile(options(), vec![Plugin::new_shared(WinningParsePlugin)])
                .await
                .unwrap();

        assert!(out.code.contains("let custom = 1;"), "{}", out.code);

        assert_eq!(out.map.expect("map").get_source(0), Some("index.ts"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_generate_hook_error_wraps_with_stage_prefix() {
        let error: CompileError =
            compile(options(), vec![Plugin::new_shared(FailingGeneratePlugin)])
                .await
                .unwrap_err();

        let message: String = error.to_string();

        assert!(
            message.contains("generate hook: `fail-generate` generate"),
            "{}",
            message
        );

        assert!(message.contains("generate boom"), "{}", message);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_generate_hook_no_map_yields_none() {
        // A winning generate hook that returns no map keeps `map = None`; no
        // identity map is fabricated.
        let out: CompileOutput =
            compile(options(), vec![Plugin::new_shared(MaplessGeneratePlugin)])
                .await
                .unwrap();

        assert_eq!(out.code, "custom();");
        assert!(out.map.is_none(), "no map when the hook returns none");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_options_hook_sees_raw_cwd() {
        // The options fixpoint runs before resolution, so the hook observes
        // the raw (unresolved) cwd.
        let observed: Arc<Mutex<Vec<Option<String>>>> =
            Arc::new(Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(ObserveOptionsPlugin {
                observed: Arc::clone(&observed),
            })];

        compile(options(), plugins).await.unwrap();

        assert_eq!(*observed.lock().unwrap(), vec![None]);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_options_hook_cannot_change_code() {
        // The options payload no longer carries `code`: a hook can retarget
        // the file but the source is untouched.
        let seen: Arc<Mutex<Vec<ResolvedOptions>>> =
            Arc::new(Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(RewriteFileOptionsPlugin),
            Plugin::new_shared(ObserveStartPlugin { seen: Arc::clone(&seen) }),
        ];

        let out: CompileOutput = compile(options(), plugins).await.unwrap();

        assert_eq!(out.code, "const a = 1;");

        let recorded: Vec<ResolvedOptions> = seen.lock().unwrap().clone();

        assert_eq!(recorded[0].file, "rewritten.ts");
        assert_eq!(recorded[0].code, "const a = 1;");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_late_options_cwd_omission_resolves_to_process_cwd() {
        let observed: Arc<Mutex<Vec<String>>> =
            Arc::new(Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(DropCwdOptionsPlugin),
            Plugin::new_shared(ObserveStartCwdPlugin {
                observed: Arc::clone(&observed),
            }),
        ];

        compile(options(), plugins).await.unwrap();

        let recorded: Vec<String> = observed.lock().unwrap().clone();

        assert_eq!(recorded, vec![resolve_process_cwd()]);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_late_options_file_omission_resolves_to_default() {
        let seen: Arc<Mutex<Vec<ResolvedOptions>>> =
            Arc::new(Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(DropFileOptionsPlugin),
            Plugin::new_shared(ObserveStartPlugin { seen: Arc::clone(&seen) }),
        ];

        compile(options(), plugins).await.unwrap();

        let recorded: Vec<ResolvedOptions> = seen.lock().unwrap().clone();

        assert_eq!(recorded[0].file, "index.js");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_options_hook_error_aborts_with_stage_prefix() {
        // A per-plugin options error composes to
        // `options hook: \`{name}\` options: {err}` — the stage prefix wraps
        // the runner's per-plugin context exactly once.
        let error: CompileError =
            compile(options(), vec![Plugin::new_shared(FailingOptionsPlugin)])
                .await
                .unwrap_err();

        let message: String = error.to_string();

        assert!(message.contains("options hook"), "{}", message);
        assert!(message.contains("`failing-options` options"), "{}", message);
        assert!(message.contains("stage boom"), "{}", message);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_options_injected_transform_plugin_runs() {
        // A plugin injected via the `options` fixpoint joins the settled list
        // and runs (its transform hook renames `console`).
        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(InjectOptionsPlugin {
                injected: Plugin::new_shared(RenameCalleePlugin),
                remaining: Mutex::new(1),
            })];

        let out: CompileOutput =
            compile(code_options("console.log(1);"), plugins).await.unwrap();

        assert!(out.code.contains("consolex"), "{}", out.code);

        assert_eq!(out.map.expect("map").get_source(0), Some("index.ts"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_parse_always_runs_even_with_injected_prepare_plugin()
    {
        // An options-only plugin injects a PRE-declaring plugin. Parse still
        // always runs, so invalid syntax now errors.
        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(InjectOptionsPlugin {
                injected: Plugin::new_shared(NoopPlugin),
                remaining: Mutex::new(1),
            })];

        let error: CompileError =
            compile(code_options("const = ;"), plugins).await.unwrap_err();

        assert!(error.to_string().contains("index.ts"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_start_sees_fully_resolved_options() {
        let seen: Arc<Mutex<Vec<ResolvedOptions>>> =
            Arc::new(Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(NoopPlugin),
            Plugin::new_shared(ObserveStartPlugin { seen: Arc::clone(&seen) }),
        ];

        compile(options(), plugins).await.unwrap();

        let recorded: Vec<ResolvedOptions> = seen.lock().unwrap().clone();

        assert_eq!(recorded.len(), 1, "start ran once");

        let resolved: &ResolvedOptions = &recorded[0];

        assert_eq!(
            resolved.cwd,
            std::env::current_dir()
                .map(|p: std::path::PathBuf| p.to_string_lossy().into_owned())
                .unwrap_or_else(|_| String::from("/"))
                .as_str(),
        );

        assert_eq!(resolved.file, "index.ts");
        assert_eq!(resolved.code, "const a = 1;");
        assert_eq!(resolved.language, Language::TS);

        assert_eq!(
            resolved.plugins,
            vec![String::from("noop"), String::from("observe-start"),],
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_start_sees_resolved_source_type_cjs() {
        let seen: Arc<Mutex<Vec<ResolvedOptions>>> =
            Arc::new(Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(ObserveStartPlugin {
                seen: Arc::clone(&seen),
            })];

        let opts: CompileOptions = CompileOptions {
            file: "index.cjs".to_string(),
            code: "module.exports = 1;".to_string(),
            ..Default::default()
        };

        compile(opts, plugins).await.unwrap();

        let recorded: Vec<ResolvedOptions> = seen.lock().unwrap().clone();

        assert_eq!(recorded[0].language, Language::JS);
        assert_eq!(recorded[0].source_type, SourceType::CommonJS);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_duplicate_names_reach_resolved_options() {
        // Duplicate plugin names are allowed everywhere (no dedup): two
        // plugins with the same name both run, and `ResolvedOptions.plugins`
        // carries BOTH names in settled order.
        let observer: Arc<Mutex<Vec<ResolvedOptions>>> =
            Arc::new(Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(StartObserverPlugin {
                name: "twin",
                observer: Arc::clone(&observer),
            }),
            Plugin::new_shared(StartObserverPlugin {
                name: "twin",
                observer: Arc::clone(&observer),
            }),
        ];

        compile(options(), plugins).await.unwrap();

        let recorded: Vec<ResolvedOptions> = observer.lock().unwrap().clone();

        assert_eq!(recorded.len(), 2, "each twin's start ran");

        assert_eq!(recorded[0].plugins, vec![String::from("twin"); 2]);
        assert_eq!(recorded[1].plugins, vec![String::from("twin"); 2]);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_prepare_observers_see_parsed_ast() {
        let seen: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(ObservePreparePlugin {
                seen: Arc::clone(&seen),
            })];

        compile(code_options("console.log(1);"), plugins).await.unwrap();

        assert_eq!(
            *seen.lock().unwrap(),
            vec![String::from("console.log(1);")]
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_finalize_observers_see_final_ast() {
        // With a transform that replaces the AST, finalize observes the
        // replaced AST (not the parse result).
        let seen: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(ReplaceRootWithConsolePlugin),
            Plugin::new_shared(ObserveFinalizePlugin {
                seen: Arc::clone(&seen),
            }),
        ];

        compile(code_options("console.log(1);"), plugins).await.unwrap();

        let recorded: Vec<String> = seen.lock().unwrap().clone();

        assert_eq!(recorded.len(), 1);
        assert_eq!(recorded[0], REPLACED_CODE);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_prepare_and_finalize_observe_same_ast_by_reference() {
        // No transform changes the AST, so prepare and finalize observe the
        // same parsed AST by reference.
        let seen: Arc<Mutex<Vec<usize>>> = Arc::new(Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(ObserveAstAddressPlugin {
                usage: HookUsage::Prepare | HookUsage::Finalize,
                seen: Arc::clone(&seen),
            })];

        compile(code_options("console.log(1);"), plugins).await.unwrap();

        let recorded: Vec<usize> = seen.lock().unwrap().clone();

        assert_eq!(recorded.len(), 2);
        assert_eq!(
            recorded[0], recorded[1],
            "prepare and finalize must observe the same AST by reference"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_prepare_observation_does_not_change_output() {
        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(ObservePreparePlugin {
                seen: Arc::new(Mutex::new(Vec::new())),
            })];

        let with_observer: CompileOutput =
            compile(code_options("console.log(1);"), plugins).await.unwrap();

        let plain: CompileOutput =
            compile(code_options("console.log(1);"), vec![]).await.unwrap();

        assert_eq!(with_observer.code, plain.code);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_finalize_observation_does_not_change_output() {
        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(ObserveFinalizePlugin {
                seen: Arc::new(Mutex::new(Vec::new())),
            })];

        let with_observer: CompileOutput =
            compile(code_options("console.log(1);"), plugins).await.unwrap();

        let plain: CompileOutput =
            compile(code_options("console.log(1);"), vec![]).await.unwrap();

        assert_eq!(with_observer.code, plain.code);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_end_runs_on_success() {
        let seen: Arc<Mutex<Vec<Option<String>>>> =
            Arc::new(Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(RenameCalleePlugin),
            Plugin::new_shared(EndObserverPlugin { seen: Arc::clone(&seen) }),
        ];

        compile(code_options("console.log(1);"), plugins).await.unwrap();

        assert_eq!(*seen.lock().unwrap(), vec![None]);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_end_runs_on_prepare_error_path() {
        let seen: Arc<Mutex<Vec<Option<String>>>> =
            Arc::new(Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(FailingPreparePlugin),
            Plugin::new_shared(EndObserverPlugin { seen: Arc::clone(&seen) }),
        ];

        let error: CompileError =
            compile(options(), plugins).await.unwrap_err();

        assert!(error.to_string().contains("prepare hook"));

        let recorded: Vec<Option<String>> = seen.lock().unwrap().clone();

        assert_eq!(recorded.len(), 1, "end ran on the error path");

        assert!(
            recorded[0]
                .as_deref()
                .is_some_and(|err| err.contains("prepare hook")),
            "{:?}",
            recorded[0]
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_end_runs_on_parse_error_path() {
        let seen: Arc<Mutex<Vec<Option<String>>>> =
            Arc::new(Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(EndObserverPlugin {
                seen: Arc::clone(&seen),
            })];

        let error: CompileError =
            compile(code_options("const = ;"), plugins).await.unwrap_err();

        assert!(error.to_string().contains("index.ts"));

        let recorded: Vec<Option<String>> = seen.lock().unwrap().clone();

        assert_eq!(recorded.len(), 1, "end ran on the parse-error path");
        assert!(recorded[0].is_some());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_end_runs_on_transform_error_path() {
        let seen: Arc<Mutex<Vec<Option<String>>>> =
            Arc::new(Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(FailingTransformHookPlugin),
            Plugin::new_shared(EndObserverPlugin { seen: Arc::clone(&seen) }),
        ];

        let error: CompileError =
            compile(options(), plugins).await.unwrap_err();

        assert!(error.to_string().contains("transform hook:"));

        let recorded: Vec<Option<String>> = seen.lock().unwrap().clone();

        assert_eq!(recorded.len(), 1);
        assert!(recorded[0].is_some());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_end_runs_on_finalize_error_path() {
        let seen: Arc<Mutex<Vec<Option<String>>>> =
            Arc::new(Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(FailingFinalizePlugin),
            Plugin::new_shared(EndObserverPlugin { seen: Arc::clone(&seen) }),
        ];

        let error: CompileError =
            compile(options(), plugins).await.unwrap_err();

        assert!(error.to_string().contains("finalize hook"));

        let recorded: Vec<Option<String>> = seen.lock().unwrap().clone();

        assert_eq!(recorded.len(), 1);
        assert!(recorded[0].is_some());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_options_error_path_does_not_run_end() {
        // The options fixpoint aborts BEFORE the driver exists — the settled
        // list is unknowable, so `end` cannot run for this path.
        let seen: Arc<Mutex<Vec<Option<String>>>> =
            Arc::new(Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(FailingOptionsPlugin),
            Plugin::new_shared(EndObserverPlugin { seen: Arc::clone(&seen) }),
        ];

        let error: CompileError =
            compile(options(), plugins).await.unwrap_err();

        assert!(error.to_string().contains("options hook"));

        assert!(
            seen.lock().unwrap().is_empty(),
            "end must not run when the options fixpoint fails"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_end_error_on_success_path_aborts() {
        let error: CompileError =
            compile(options(), vec![Plugin::new_shared(FailingEndPlugin)])
                .await
                .unwrap_err();

        let message: String = error.to_string();

        assert!(message.contains("end hook"), "{}", message);
        assert!(message.contains("`failing-end` end"), "{}", message);
        assert!(message.contains("end boom"), "{}", message);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_end_error_on_error_path_original_wins() {
        // A failing stage AND a failing `end`: the original error wins, with
        // the `end` failure appended (nothing swallowed).
        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(FailingPreparePlugin),
            Plugin::new_shared(FailingEndPlugin),
        ];

        let error: CompileError =
            compile(options(), plugins).await.unwrap_err();

        let message: String = error.to_string();

        assert!(
            message.starts_with(
                "prepare hook: `failing-prepare` prepare: stage boom"
            ),
            "{}",
            message
        );

        assert!(message.contains("(end hook failed:"), "{}", message);

        // The appended driver error is context-wrapped with the failing
        // plugin's name.
        assert!(message.contains("`failing-end` end"), "{}", message);
    }
}
