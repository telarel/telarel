mod ordering;
mod stages;

use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;

use oxc::ast::ast::{Directive, Program, StringLiteral};
use oxc::ast::builder::AstBuilder;
use oxc::span::SPAN;

use telarel_common::{
    Ast, AstResult, CodeResult, Generator, HookUsage, Language,
    ParseAstOptions, Parser, ResolvedGeneratorOptions,
    ResolvedGeneratorStageOptions, ResolvedParserOptions,
    ResolvedParserStageOptions, SourceMapOptions, SourceType, parse_ast,
};

use crate::_types::context::{CommonPluginContext, ModuleInfo};
use crate::_types::hooks::finalize::FinalizeArgs;
use crate::_types::hooks::generate::{GenerateArgs, GenerateReturn};
use crate::_types::hooks::notify::NotifyReturn;
use crate::_types::hooks::options::{OptionsArgs, OptionsReturn};
use crate::_types::hooks::parse::{ParseArgs, ParseReturn};
use crate::_types::hooks::prepare::PrepareArgs;
use crate::_types::hooks::start::StartArgs;
use crate::_types::hooks::transform::{TransformArgs, TransformReturn};
use crate::_types::order::{PluginHookMeta, PluginOrder};
use crate::SharedPluginable;
use crate::plugin::Plugin;
use crate::plugin::pluginable::{
    HookFuture, Pluginable, SharedTransformBatch, TransformBatch,
    TransformBatchable,
};

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

fn parser_options(file: &str) -> ResolvedParserOptions {
    let stage: ResolvedParserStageOptions =
        ResolvedParserStageOptions::default();

    ResolvedParserOptions {
        cwd: PathBuf::from("/repo"),
        file: file.to_string(),
        language: Language::TS,
        source_type: SourceType::Module,
        source_map: SourceMapOptions::default(),
        preserve_parens: stage.preserve_parens,
        allow_return_outside_function: stage.allow_return_outside_function,
        oxc: stage.oxc,
    }
}

fn generator_options(file: &str) -> ResolvedGeneratorOptions {
    let stage: ResolvedGeneratorStageOptions =
        ResolvedGeneratorStageOptions::default();

    ResolvedGeneratorOptions {
        cwd: PathBuf::from("/repo"),
        file: file.to_string(),
        source_map: SourceMapOptions::default(),
        single_quote: stage.single_quote,
        ascii_only: stage.ascii_only,
        indent_style: stage.indent_style,
        indent_width: stage.indent_width,
        comments: stage.comments,
        minify: stage.minify,
        oxc: stage.oxc,
    }
}

fn code_result(code: &str) -> CodeResult {
    CodeResult { code: code.to_string(), map: None }
}

fn probe_counts(probe: &BatchProbe) -> (usize, usize, usize) {
    (
        *probe.single_calls.lock().unwrap(),
        *probe.make_calls.lock().unwrap(),
        *probe.batch_calls.lock().unwrap(),
    )
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
        let mut seen: MutexGuard<'_, Vec<usize>> = self.seen.lock().unwrap();
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

                let string_literal: StringLiteral<'_> =
                    StringLiteral::new(SPAN, APPEND_DIRECTIVE, None, &builder);

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

/// Counters shared by the test-only batchable plugins.
#[derive(Default)]
struct BatchProbe {
    single_calls: Mutex<usize>,
    make_calls: Mutex<usize>,
    batch_calls: Mutex<usize>,
    members: Mutex<Vec<Vec<String>>>,
}

/// The outcome a test batch reports.
#[derive(Debug, Clone, Copy)]
enum BatchOutcome {
    None,
    Replace(&'static str),
    Fail,
}

/// A test-only transform batch with a fixed outcome.
struct ProbeBatch {
    outcome: BatchOutcome,
    probe: Arc<BatchProbe>,
}

impl TransformBatch for ProbeBatch {
    fn call_transform<'a>(
        &'a self,
        _ctx: &'a PluginContext<'_>,
        _args: TransformArgs,
    ) -> HookFuture<'a, TransformReturn> {
        Box::pin(async move {
            *self.probe.batch_calls.lock().unwrap() += 1;

            match self.outcome {
                | BatchOutcome::None => Ok(None),
                | BatchOutcome::Replace(code) => {
                    let ast: Ast = parse_fixture(code)?;

                    Ok(Some(crate::TransformOutput { ast }))
                },
                | BatchOutcome::Fail => Err(PluginError::new("batch-boom")),
            }
        })
    }
}

/// A test-only plugin that implements [`Pluginable`] directly, so it can
/// carry the internal transform batch capability.
struct BatchableTestPlugin {
    name: &'static str,
    meta: Option<PluginHookMeta>,
    batchable: bool,
    batch: Option<SharedTransformBatch>,
    probe: Arc<BatchProbe>,
}

impl BatchProbe {
    fn batchable(
        self: &Arc<Self>,
        name: &'static str,
        meta: Option<PluginHookMeta>,
        outcome: BatchOutcome,
    ) -> SharedPluginable {
        Arc::new(BatchableTestPlugin {
            name,
            meta,
            batchable: true,
            batch: Some(Arc::new(ProbeBatch {
                outcome,
                probe: Arc::clone(self),
            })),
            probe: Arc::clone(self),
        })
    }

    fn batchable_without_batch(
        self: &Arc<Self>,
        name: &'static str,
        meta: Option<PluginHookMeta>,
    ) -> SharedPluginable {
        Arc::new(BatchableTestPlugin {
            name,
            meta,
            batchable: true,
            batch: None,
            probe: Arc::clone(self),
        })
    }

    fn plain(
        self: &Arc<Self>,
        name: &'static str,
        meta: Option<PluginHookMeta>,
    ) -> SharedPluginable {
        Arc::new(BatchableTestPlugin {
            name,
            meta,
            batchable: false,
            batch: None,
            probe: Arc::clone(self),
        })
    }
}

impl Pluginable for BatchableTestPlugin {
    fn call_name(&self) -> Cow<'static, str> {
        self.name.into()
    }

    fn call_register_hook_usage(&self) -> HookUsage {
        HookUsage::Transform
    }

    fn call_options<'a>(
        &'a self,
        _ctx: &'a CommonPluginContext,
        _args: &'a OptionsArgs,
    ) -> HookFuture<'a, OptionsReturn> {
        Box::pin(async { Ok(None) })
    }

    fn call_start<'a>(
        &'a self,
        _ctx: &'a PluginContext<'_>,
        _args: &'a StartArgs,
    ) -> HookFuture<'a, NotifyReturn> {
        Box::pin(async { Ok(()) })
    }

    fn call_parse<'a>(
        &'a self,
        _ctx: &'a PluginContext<'_>,
        _args: ParseArgs<'a>,
    ) -> HookFuture<'a, ParseReturn> {
        Box::pin(async { Ok(None) })
    }

    fn call_prepare<'a>(
        &'a self,
        _ctx: &'a PluginContext<'_>,
        _args: PrepareArgs<'a>,
    ) -> HookFuture<'a, NotifyReturn> {
        Box::pin(async { Ok(()) })
    }

    fn call_transform<'a>(
        &'a self,
        _ctx: &'a PluginContext<'_>,
        _args: TransformArgs,
    ) -> HookFuture<'a, TransformReturn> {
        Box::pin(async move {
            *self.probe.single_calls.lock().unwrap() += 1;

            Ok(None)
        })
    }

    fn call_transform_meta(&self) -> Option<PluginHookMeta> {
        self.meta
    }

    fn as_transform_batchable(&self) -> Option<&dyn TransformBatchable> {
        if self.batchable { Some(self) } else { None }
    }

    fn call_finalize<'a>(
        &'a self,
        _ctx: &'a PluginContext<'_>,
        _args: FinalizeArgs<'a>,
    ) -> HookFuture<'a, NotifyReturn> {
        Box::pin(async { Ok(()) })
    }

    fn call_generate<'a>(
        &'a self,
        _ctx: &'a PluginContext<'_>,
        _args: GenerateArgs<'a>,
    ) -> HookFuture<'a, GenerateReturn> {
        Box::pin(async { Ok(None) })
    }

    fn call_end<'a>(
        &'a self,
        _ctx: &'a PluginContext<'_>,
        _args: &'a crate::EndArgs,
    ) -> HookFuture<'a, NotifyReturn> {
        Box::pin(async { Ok(()) })
    }
}

impl TransformBatchable for BatchableTestPlugin {
    fn make_transform_batch(
        &self,
        members: &[&dyn Pluginable],
    ) -> Option<SharedTransformBatch> {
        *self.probe.make_calls.lock().unwrap() += 1;

        self.probe.members.lock().unwrap().push(
            members
                .iter()
                .map(|member| member.call_name().into_owned())
                .collect(),
        );

        self.batch.clone()
    }
}

/// Records the source text of the AST it observes and reports no change.
#[derive(Debug)]
struct ObserveCarriedAstPlugin {
    seen: Arc<Mutex<Vec<String>>>,
}

impl Plugin for ObserveCarriedAstPlugin {
    fn name(&self) -> Cow<'static, str> {
        "observe-carried-ast".into()
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
            self.seen.lock().unwrap().push(args.ast.source().to_string());

            Ok(None)
        }
    }
}

/// Records every invocation under `name` and can fail on request.
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

            if self.fail { Err(PluginError::new(self.name)) } else { Ok(()) }
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

            if self.fail { Err(PluginError::new(self.name)) } else { Ok(()) }
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
            parser: ResolvedParserStageOptions::default(),
            generator: ResolvedGeneratorStageOptions::default(),
            output: telarel_common::ResolvedOutputOptions {
                file: String::from("dist/a.js"),
            },
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
            [args.code.to_string(), args.options.file.to_string()].join("|");

        async move {
            self.seen.lock().unwrap().push(observed);

            Ok(None)
        }
    }
}

/// Implements `parse` but declares a non-`Parse` usage. The driver must
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
