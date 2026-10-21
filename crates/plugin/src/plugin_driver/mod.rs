pub mod hooks;

use std::sync::Arc;

use telarel_common::{
    Ast, AstResult, CodeResult, CompileError, Generator, GeneratorOptions,
    HookUsage, Parser, ParserOptions,
};

use crate::_types::context::PluginContext;
use crate::_types::hooks::end::EndArgs;
use crate::_types::hooks::finalize::FinalizeArgs;
use crate::_types::hooks::prepare::PrepareArgs;
use crate::_types::hooks::start::StartArgs;
use crate::_types::hooks::transform::{TransformArgs, TransformOutput};
use crate::_types::order::{PluginHookMeta, PluginOrder};
use crate::PluginError;
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
    start_plugins: Vec<SharedPluginable>,
    parse_plugins: Vec<SharedPluginable>,
    prepare_plugins: Vec<SharedPluginable>,
    transform_plugins: Vec<SharedPluginable>,
    finalize_plugins: Vec<SharedPluginable>,
    generate_plugins: Vec<SharedPluginable>,
    end_plugins: Vec<SharedPluginable>,
}

impl PluginDriver {
    /// Create a driver from plugins in registration order.
    ///
    /// `start`/`end` run on every settled plugin; `parse`, `prepare`,
    /// `transform`, `finalize`, and `generate` are partitioned by declared
    /// [`HookUsage`]. `parse` and `generate` are first-win, `transform` carries
    /// the last `Some`, and `prepare`/`finalize` are notify-only observers.
    /// `options` is not a driver partition: it is the fixpoint runner
    /// ([`crate::options_fixpoint`]).
    pub fn new(plugins: Vec<SharedPluginable>) -> Self {
        let mut usage: HookUsage = HookUsage::default();

        let mut parse_plugins: Vec<SharedPluginable> = Vec::new();
        let mut prepare_plugins: Vec<SharedPluginable> = Vec::new();
        let mut transform_plugins: Vec<SharedPluginable> = Vec::new();
        let mut finalize_plugins: Vec<SharedPluginable> = Vec::new();
        let mut generate_plugins: Vec<SharedPluginable> = Vec::new();

        for plugin in &plugins {
            let declared: HookUsage = plugin.call_register_hook_usage();

            usage |= declared;

            if declared.contains(HookUsage::Parse) {
                parse_plugins.push(Arc::clone(plugin));
            }

            if declared.contains(HookUsage::Prepare) {
                prepare_plugins.push(Arc::clone(plugin));
            }

            if declared.contains(HookUsage::Transform) {
                transform_plugins.push(Arc::clone(plugin));
            }

            if declared.contains(HookUsage::Finalize) {
                finalize_plugins.push(Arc::clone(plugin));
            }

            if declared.contains(HookUsage::Generate) {
                generate_plugins.push(Arc::clone(plugin));
            }
        }

        let start_plugins: Vec<SharedPluginable> =
            sort_plugins_by_hook_meta(&plugins, |plugin| {
                plugin.call_start_meta()
            });

        let parse_plugins: Vec<SharedPluginable> =
            sort_plugins_by_hook_meta(&parse_plugins, |plugin| {
                plugin.call_parse_meta()
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

        let generate_plugins: Vec<SharedPluginable> =
            sort_plugins_by_hook_meta(&generate_plugins, |plugin| {
                plugin.call_generate_meta()
            });

        let end_plugins: Vec<SharedPluginable> =
            sort_plugins_by_hook_meta(&plugins, |plugin| {
                plugin.call_end_meta()
            });

        let settled_names: Vec<String> = plugins
            .iter()
            .map(|plugin| plugin.call_name().into_owned())
            .collect();

        Self {
            usage,
            settled_names,
            start_plugins,
            parse_plugins,
            prepare_plugins,
            transform_plugins,
            finalize_plugins,
            generate_plugins,
            end_plugins,
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

    /// Run the `start` hook on EVERY settled plugin (not partitioned by
    /// usage); the first error aborts. Notify-only.
    pub async fn start(
        &self,
        ctx: &PluginContext<'_>,
        args: &StartArgs,
    ) -> Result<(), PluginError> {
        hooks::start::start(&self.start_plugins, ctx, args).await
    }

    /// Run the `parse` hook over the `parse` partition.
    ///
    /// First-win: the first `Ok(Some)` result wins and the remaining hooks
    /// are skipped; `Ok(None)` defers to the next plugin. The first error
    /// aborts. When every hook defers, `builtin` runs and its raw error is
    /// returned.
    pub async fn parse(
        &self,
        ctx: &PluginContext<'_>,
        code: &str,
        options: &ParserOptions,
        builtin: &Parser,
    ) -> Result<AstResult, CompileError> {
        hooks::parse::parse(&self.parse_plugins, ctx, code, options, builtin)
            .await
    }

    /// Run the `prepare` hook over the `prepare` partition in meta-ranked
    /// order, as a read-only observer of the parsed AST; the first error
    /// aborts. Return values are ignored.
    pub async fn prepare<'a>(
        &'a self,
        ctx: &'a PluginContext<'_>,
        args: PrepareArgs<'a>,
    ) -> Result<(), PluginError> {
        hooks::prepare::prepare(&self.prepare_plugins, ctx, args).await
    }

    /// Run the `transform` hook chain; `Some` carries the last returned AST,
    /// `None` = unchanged.
    pub async fn transform(
        &self,
        ctx: &PluginContext<'_>,
        args: TransformArgs,
    ) -> Result<Option<TransformOutput>, PluginError> {
        hooks::transform::transform(&self.transform_plugins, ctx, args).await
    }

    /// Run the `finalize` hook over the `finalize` partition in meta-ranked
    /// order, as a read-only observer of the final AST; the first error
    /// aborts. Return values are ignored.
    pub async fn finalize<'a>(
        &'a self,
        ctx: &'a PluginContext<'_>,
        args: FinalizeArgs<'a>,
    ) -> Result<(), PluginError> {
        hooks::finalize::finalize(&self.finalize_plugins, ctx, args).await
    }

    /// Run the `generate` hook over the `generate` partition.
    ///
    /// First-win: the first `Ok(Some)` result wins and the remaining hooks
    /// are skipped; `Ok(None)` defers to the next plugin. The first error
    /// aborts. When every hook defers, `builtin` runs and its raw error is
    /// returned.
    pub async fn generate(
        &self,
        ctx: &PluginContext<'_>,
        ast: &Ast,
        options: &GeneratorOptions,
        builtin: &Generator<'_>,
    ) -> Result<CodeResult, CompileError> {
        hooks::generate::generate(
            &self.generate_plugins,
            ctx,
            ast,
            options,
            builtin,
        )
        .await
    }

    /// Run the `end` hook on EVERY settled plugin (not partitioned by usage);
    /// every hook runs even after one fails, and the first error is returned
    /// after the loop. Notify-only.
    pub async fn end(
        &self,
        ctx: &PluginContext<'_>,
        args: &EndArgs,
    ) -> Result<(), PluginError> {
        hooks::end::end(&self.end_plugins, ctx, args).await
    }
}

// The house style for the hook impls is the explicit RPITIT form (never
// `async fn`), so the `manual_async_fn` lint is suppressed here.
#[cfg(test)]
#[allow(clippy::manual_async_fn)]
mod tests {
    use std::borrow::Cow;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::sync::MutexGuard;

    use oxc::ast::ast::{Directive, Program, StringLiteral};
    use oxc::ast::builder::AstBuilder;
    use oxc::span::SPAN;

    use telarel_common::{
        Ast, AstResult, CodeResult, Generator, GeneratorOptions, HookUsage,
        Language, ParseAstOptions, Parser, ParserOptions, SourceMapOptions,
        SourceType, parse_ast,
    };

    use crate::_types::context::ModuleInfo;
    use crate::_types::hooks::notify::NotifyReturn;
    use crate::_types::hooks::parse::ParseArgs;
    use crate::_types::hooks::transform::{TransformArgs, TransformReturn};
    use crate::_types::order::{PluginHookMeta, PluginOrder};
    use crate::SharedPluginable;
    use crate::plugin::Plugin;
    use crate::plugin::pluginable::Pluginable;

    use super::*;

    fn make_module<'a>() -> ModuleInfo<'a> {
        ModuleInfo {
            file: "a.ts",
            language: Language::TS,
            source_type: SourceType::Module,
        }
    }

    fn make_plugin_ctx<'a>() -> PluginContext<'a> {
        PluginContext::new(Path::new("/repo"), make_module())
    }

    fn parse_fixture(code: &str) -> Result<Ast, PluginError> {
        parse_ast(
            code.to_string(),
            ParseAstOptions {
                cwd: PathBuf::from("/repo"),
                file: String::from("a.ts"),
                language: None,
                source_type: None,
                input_map: None,
            },
        )
        .map_err(|error: CompileError| PluginError::new(error.to_string()))
    }

    fn test_ast() -> Ast {
        parse_fixture("console.log(1);").expect("fixture parses")
    }

    fn parser_options(file: &str) -> ParserOptions {
        ParserOptions {
            cwd: PathBuf::from("/repo"),
            file: file.to_string(),
            language: Language::TS,
            source_type: SourceType::Module,
            source_map: SourceMapOptions::default(),
        }
    }

    fn generator_options(file: &str) -> GeneratorOptions {
        GeneratorOptions {
            cwd: PathBuf::from("/repo"),
            file: file.to_string(),
            source_map: SourceMapOptions::default(),
        }
    }

    fn code_result(code: &str) -> CodeResult {
        CodeResult { code: code.to_string(), map: None }
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

        fn transform<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            _args: TransformArgs,
        ) -> impl Future<Output = TransformReturn> + Send + 'a {
            async move {
                let ast: Ast = parse_fixture("\"mark\";")?;

                Ok(Some(crate::TransformOutput { ast }))
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
            ast: &Ast,
        ) {
            let mut seen: MutexGuard<'_, Vec<usize>> =
                self.seen.lock().unwrap();
            // The program lives inside the `Ast`'s boxed self_cell allocation,
            // so its address is stable across moves of the outer `Ast`.
            seen.push(std::ptr::from_ref(ast.program()) as usize);
        }
    }

    impl Plugin for RecordingPlugin {
        fn name(&self) -> Cow<'static, str> {
            "record-transform".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Transform
        }

        fn transform<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            args: TransformArgs,
        ) -> impl Future<Output = TransformReturn> + Send + 'a {
            async move {
                self.record(&args.ast);

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

        fn transform<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            _args: TransformArgs,
        ) -> impl Future<Output = TransformReturn> + Send + 'a {
            async move {
                let ast: Ast = parse_fixture("\"replaced\";")?;

                Ok(Some(crate::TransformOutput { ast }))
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

        fn transform<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            args: TransformArgs,
        ) -> impl Future<Output = TransformReturn> + Send + 'a {
            async move {
                let mut ast: Ast = args.ast;

                ast.with_mut(|allocator, program: &mut Program<'_>| {
                    let builder: AstBuilder<'_> = AstBuilder::new(allocator);

                    let string_literal: StringLiteral<'_> = StringLiteral::new(
                        SPAN,
                        APPEND_DIRECTIVE,
                        None,
                        &builder,
                    );

                    let directive: Directive<'_> = Directive::new(
                        SPAN,
                        string_literal,
                        APPEND_DIRECTIVE,
                        &builder,
                    );

                    program.directives.push(directive);
                });

                Ok(Some(crate::TransformOutput { ast }))
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

        fn transform<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            _args: TransformArgs,
        ) -> impl Future<Output = TransformReturn> + Send + 'a {
            async move { Err(PluginError::new("boom")) }
        }
    }

    #[derive(Debug)]
    struct SourcedTransformPlugin;

    impl Plugin for SourcedTransformPlugin {
        fn name(&self) -> Cow<'static, str> {
            "sourced-transform".into()
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
                    "index.ts",
                    "const a = 1;",
                    oxc::span::Span::new(6, 7),
                ))
            }
        }
    }

    /// Records every invocation under `name`; optionally fails.
    #[derive(Debug)]
    struct ObserverPlugin {
        name: &'static str,
        usage: HookUsage,
        fail: bool,
        log: Arc<Mutex<Vec<String>>>,
    }

    impl ObserverPlugin {
        fn record_or_fail(&self) -> Result<(), PluginError> {
            self.log.lock().unwrap().push(self.name.to_string());

            if self.fail { Err(PluginError::new(self.name)) } else { Ok(()) }
        }
    }

    impl Plugin for ObserverPlugin {
        fn name(&self) -> Cow<'static, str> {
            self.name.into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            self.usage
        }

        fn prepare<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            _args: PrepareArgs<'a>,
        ) -> impl Future<Output = NotifyReturn> + Send + 'a {
            async move { self.record_or_fail() }
        }

        fn finalize<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            _args: FinalizeArgs<'a>,
        ) -> impl Future<Output = NotifyReturn> + Send + 'a {
            async move { self.record_or_fail() }
        }
    }

    /// Records the observed AST program address in the given hook.
    #[derive(Debug)]
    struct ObserveAstPlugin {
        usage: HookUsage,
        seen: Arc<Mutex<Vec<usize>>>,
    }

    impl ObserveAstPlugin {
        fn record(
            &self,
            ast: &Ast,
        ) {
            self.seen
                .lock()
                .unwrap()
                .push(std::ptr::from_ref(ast.program()) as usize);
        }
    }

    impl Plugin for ObserveAstPlugin {
        fn name(&self) -> Cow<'static, str> {
            "observe-ast".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            self.usage
        }

        fn prepare<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            args: PrepareArgs<'a>,
        ) -> impl Future<Output = NotifyReturn> + Send + 'a {
            async move {
                self.record(args.ast);

                Ok(())
            }
        }

        fn finalize<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            args: FinalizeArgs<'a>,
        ) -> impl Future<Output = NotifyReturn> + Send + 'a {
            async move {
                self.record(args.ast);

                Ok(())
            }
        }
    }

    #[derive(Debug)]
    struct NotifyOrderPlugin {
        name: &'static str,
        usage: HookUsage,
        log: Arc<Mutex<Vec<String>>>,
    }

    impl NotifyOrderPlugin {
        fn record(
            &self,
            stage: &str,
        ) {
            let mut log: MutexGuard<'_, Vec<String>> = self.log.lock().unwrap();
            log.push([self.name.to_string(), stage.to_string()].join(":"));
        }
    }

    impl Plugin for NotifyOrderPlugin {
        fn name(&self) -> Cow<'static, str> {
            self.name.into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            self.usage
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
    }

    #[derive(Debug)]
    struct StartPlugin {
        name: &'static str,
        fail: bool,
        log: Arc<Mutex<Vec<String>>>,
    }

    impl Plugin for StartPlugin {
        fn name(&self) -> Cow<'static, str> {
            self.name.into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Start
        }

        fn start<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            _args: &'a crate::StartArgs,
        ) -> impl Future<Output = NotifyReturn> + Send {
            async move {
                self.log.lock().unwrap().push(self.name.to_string());

                if self.fail {
                    Err(PluginError::new(self.name))
                } else {
                    Ok(())
                }
            }
        }
    }

    #[derive(Debug)]
    struct EndPlugin {
        name: &'static str,
        fail: bool,
        log: Arc<Mutex<Vec<String>>>,
    }

    impl Plugin for EndPlugin {
        fn name(&self) -> Cow<'static, str> {
            self.name.into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::End
        }

        fn end<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            _args: &'a crate::EndArgs,
        ) -> impl Future<Output = NotifyReturn> + Send {
            async move {
                self.log.lock().unwrap().push(self.name.to_string());

                if self.fail {
                    Err(PluginError::new(self.name))
                } else {
                    Ok(())
                }
            }
        }
    }

    #[derive(Debug)]
    struct NotifyArgsPlugin {
        started: Arc<Mutex<Option<PathBuf>>>,
        ended: Arc<Mutex<Vec<Option<String>>>>,
    }

    impl Plugin for NotifyArgsPlugin {
        fn name(&self) -> Cow<'static, str> {
            "notify-args".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Start | HookUsage::End
        }

        fn start<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            args: &'a crate::StartArgs,
        ) -> impl Future<Output = NotifyReturn> + Send {
            let cwd: PathBuf = args.options.cwd.clone();

            async move {
                *self.started.lock().unwrap() = Some(cwd);

                Ok(())
            }
        }

        fn end<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            args: &'a crate::EndArgs,
        ) -> impl Future<Output = NotifyReturn> + Send {
            let rendered: Option<String> =
                args.error.as_ref().map(ToString::to_string);

            async move {
                self.ended.lock().unwrap().push(rendered);

                Ok(())
            }
        }
    }

    fn make_start_args() -> crate::StartArgs {
        crate::StartArgs {
            options: telarel_common::ResolvedOptions {
                cwd: PathBuf::from("/repo"),
                file: String::from("a.ts"),
                language: Language::TS,
                source_type: SourceType::Module,
                source_map: SourceMapOptions::default(),
                plugins: vec![],
            },
        }
    }

    fn make_end_args() -> crate::EndArgs {
        crate::EndArgs { error: None }
    }

    #[derive(Debug, Clone, Copy)]
    enum ParseOutcome {
        Defer,
        Win(&'static str),
        Fail,
    }

    #[derive(Debug)]
    struct ParsePlugin {
        name: &'static str,
        outcome: ParseOutcome,
        log: Arc<Mutex<Vec<String>>>,
    }

    impl ParsePlugin {
        fn new(
            name: &'static str,
            outcome: ParseOutcome,
            log: &Arc<Mutex<Vec<String>>>,
        ) -> Self {
            Self { name, outcome, log: Arc::clone(log) }
        }

        fn record(&self) {
            self.log.lock().unwrap().push(self.name.to_string());
        }
    }

    impl Plugin for ParsePlugin {
        fn name(&self) -> Cow<'static, str> {
            self.name.into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Parse
        }

        fn parse<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            _args: ParseArgs<'a>,
        ) -> impl Future<Output = crate::ParseReturn> + Send + 'a {
            async move {
                self.record();

                match self.outcome {
                    | ParseOutcome::Defer => Ok(None),
                    | ParseOutcome::Win(code) => {
                        let ast: Ast = parse_fixture(code)?;

                        Ok(Some(AstResult { ast }))
                    },
                    | ParseOutcome::Fail => Err(PluginError::new(self.name)),
                }
            }
        }
    }

    #[derive(Debug)]
    struct SourcedParsePlugin;

    impl Plugin for SourcedParsePlugin {
        fn name(&self) -> Cow<'static, str> {
            "sourced-parse".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Parse
        }

        fn parse<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            _args: ParseArgs<'a>,
        ) -> impl Future<Output = crate::ParseReturn> + Send + 'a {
            async move {
                Err(PluginError::new("boom").with_source_code(
                    "index.ts",
                    "const a = 1;",
                    oxc::span::Span::new(6, 7),
                ))
            }
        }
    }

    /// Records the code and options file it was handed.
    #[derive(Debug)]
    struct ParseObservePlugin {
        seen: Arc<Mutex<Vec<String>>>,
    }

    impl Plugin for ParseObservePlugin {
        fn name(&self) -> Cow<'static, str> {
            "parse-observe".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Parse
        }

        fn parse<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            args: ParseArgs<'a>,
        ) -> impl Future<Output = crate::ParseReturn> + Send + 'a {
            let observed: String =
                [args.code.to_string(), args.options.file.to_string()]
                    .join("|");

            async move {
                self.seen.lock().unwrap().push(observed);

                Ok(None)
            }
        }
    }

    /// Implements `parse` but declares a non-`Parse` usage; the driver must
    /// never call it.
    #[derive(Debug)]
    struct ParseWithOtherUsagePlugin {
        name: &'static str,
        log: Arc<Mutex<Vec<String>>>,
    }

    impl Plugin for ParseWithOtherUsagePlugin {
        fn name(&self) -> Cow<'static, str> {
            self.name.into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Prepare
        }

        fn parse<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            _args: ParseArgs<'a>,
        ) -> impl Future<Output = crate::ParseReturn> + Send + 'a {
            async move {
                self.log.lock().unwrap().push(self.name.to_string());

                let ast: Ast = parse_fixture("\"ignored\";")?;

                Ok(Some(AstResult { ast }))
            }
        }
    }

    #[derive(Debug, Clone, Copy)]
    enum GenerateOutcome {
        Defer,
        Win(&'static str),
        Fail,
    }

    #[derive(Debug)]
    struct GeneratePlugin {
        name: &'static str,
        outcome: GenerateOutcome,
        log: Arc<Mutex<Vec<String>>>,
    }

    impl GeneratePlugin {
        fn new(
            name: &'static str,
            outcome: GenerateOutcome,
            log: &Arc<Mutex<Vec<String>>>,
        ) -> Self {
            Self { name, outcome, log: Arc::clone(log) }
        }

        fn record(&self) {
            self.log.lock().unwrap().push(self.name.to_string());
        }
    }

    impl Plugin for GeneratePlugin {
        fn name(&self) -> Cow<'static, str> {
            self.name.into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Generate
        }

        fn generate<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            _args: crate::GenerateArgs<'a>,
        ) -> impl Future<Output = crate::GenerateReturn> + Send + 'a {
            async move {
                self.record();

                match self.outcome {
                    | GenerateOutcome::Defer => Ok(None),
                    | GenerateOutcome::Win(code) => Ok(Some(code_result(code))),
                    | GenerateOutcome::Fail => Err(PluginError::new(self.name)),
                }
            }
        }
    }

    /// Records the AST source and options file it was handed.
    #[derive(Debug)]
    struct GenerateObservePlugin {
        seen: Arc<Mutex<Vec<String>>>,
    }

    impl Plugin for GenerateObservePlugin {
        fn name(&self) -> Cow<'static, str> {
            "generate-observe".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Generate
        }

        fn generate<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            args: crate::GenerateArgs<'a>,
        ) -> impl Future<Output = crate::GenerateReturn> + Send + 'a {
            let observed: String =
                [args.ast.source().to_string(), args.options.file.to_string()]
                    .join("|");

            async move {
                self.seen.lock().unwrap().push(observed);

                Ok(None)
            }
        }
    }

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

        let options: ParserOptions = parser_options("a.ts");

        let builtin: Parser =
            Parser::new(String::from("let builtin;"), parser_options("a.ts"));

        let result: AstResult = driver
            .parse(&ctx, "let builtin;", &options, &builtin)
            .await
            .unwrap();

        assert_eq!(result.ast.source().as_ref(), "let first;");

        assert_eq!(*log.lock().unwrap(), vec![String::from("first")]);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_parse_null_defer_chain_falls_back_to_builtin() {
        let ctx: PluginContext<'_> = make_plugin_ctx();

        let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(ParsePlugin::new(
                "one",
                ParseOutcome::Defer,
                &log,
            )),
            Plugin::new_shared(ParsePlugin::new(
                "two",
                ParseOutcome::Defer,
                &log,
            )),
        ];

        let driver: PluginDriver = PluginDriver::new(plugins);

        let options: ParserOptions = parser_options("a.ts");

        let builtin: Parser =
            Parser::new(String::from("let builtin;"), parser_options("a.ts"));

        let result: AstResult = driver
            .parse(&ctx, "let builtin;", &options, &builtin)
            .await
            .unwrap();

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
            Plugin::new_shared(ParsePlugin::new(
                "one",
                ParseOutcome::Defer,
                &log,
            )),
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

        let options: ParserOptions = parser_options("a.ts");

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

        let options: ParserOptions = parser_options("index.ts");

        let builtin: Parser = Parser::new(
            String::from("const a = 1;"),
            parser_options("index.ts"),
        );

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

        let options: ParserOptions = parser_options("index.ts");

        let builtin: Parser =
            Parser::new(String::from("const = ;"), parser_options("index.ts"));

        let err: CompileError = driver
            .parse(&ctx, "const = ;", &options, &builtin)
            .await
            .unwrap_err();

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

        let options: ParserOptions = parser_options("a.ts");

        let builtin: Parser =
            Parser::new(String::from("let builtin;"), parser_options("a.ts"));

        let result: AstResult = driver
            .parse(&ctx, "let builtin;", &options, &builtin)
            .await
            .unwrap();

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

        let options: ParserOptions = parser_options("input.ts");

        let builtin: Parser =
            Parser::new(String::from("let a;"), parser_options("input.ts"));

        driver.parse(&ctx, "let a;", &options, &builtin).await.unwrap();

        assert_eq!(
            *seen.lock().unwrap(),
            vec![String::from("let a;|input.ts")]
        );
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

        let options: GeneratorOptions = generator_options("a.ts");

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

        let options: GeneratorOptions = generator_options("a.ts");

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

        let options: GeneratorOptions = generator_options("a.ts");

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

        let options: GeneratorOptions = generator_options("input.ts");

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

        let err: PluginError = driver
            .finalize(&ctx, FinalizeArgs { ast: &ast })
            .await
            .unwrap_err();

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

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(MarkPlugin),
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

        let first_seen: Arc<Mutex<Vec<usize>>> =
            Arc::new(Mutex::new(Vec::new()));

        let second_seen: Arc<Mutex<Vec<usize>>> =
            Arc::new(Mutex::new(Vec::new()));

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

    #[tokio::test(flavor = "current_thread")]
    async fn test_pluginable_dispatch() {
        let plugin: Arc<dyn Pluginable> = Plugin::new_shared(MarkPlugin);

        assert_eq!(plugin.call_name(), "mark");

        assert!(
            plugin.call_register_hook_usage().contains(HookUsage::Transform)
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_driver_aggregates_usage() {
        // An options-only plugin still contributes its declared bit to the
        // aggregate (no options partition exists — the fixpoint runner owns
        // the options stage).
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

        assert_eq!(
            started.lock().unwrap().as_deref(),
            Some(Path::new("/repo"))
        );
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

        assert_eq!(
            *ended.lock().unwrap(),
            vec![None, Some(String::from("boom"))]
        );
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
                MetaOrderPlugin::new("b", &log)
                    .with_prepare_meta(PluginOrder::Pre),
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

        let names: Vec<String> = ranked
            .iter()
            .map(|plugin| plugin.call_name().into_owned())
            .collect();

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

        let names: Vec<String> = ranked
            .iter()
            .map(|plugin| plugin.call_name().into_owned())
            .collect();

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
}
