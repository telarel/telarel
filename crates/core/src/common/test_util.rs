// The house style for the test hook impls is the explicit RPITIT form (never
// `async fn`), so the `manual_async_fn` lint is suppressed here.
#![allow(clippy::manual_async_fn)]

use std::borrow::Cow;
use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;

use oxc_sourcemap::SourceMapBuilder;
use tempfile::TempDir;

use telarel_common::{
    Ast, AstResult, CodeResult, InputSourceMap, InputSourceMapMode,
    OutputSourceMap, OutputSourceMapMode, ParseAstOptions, ResolvedOptions,
    SourceMap, SourceMapOptions, parse_ast,
};
use telarel_plugin::{
    CommonPluginContext, EndArgs, FinalizeArgs, GenerateArgs, GenerateReturn,
    HookUsage, NotifyReturn, OptionsArgs, OptionsReturn, ParseArgs,
    ParseReturn, Plugin, PluginContext, PrepareArgs, StartArgs, TransformArgs,
    TransformOutput, TransformReturn,
};

pub fn parse_fixture(code: &str) -> anyhow::Result<Ast> {
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

pub fn parse_with_map(
    code: &str,
    map: SourceMap,
) -> Ast {
    parse_ast(
        code.to_string(),
        ParseAstOptions {
            cwd: String::from("/repo"),
            file: String::from("index.ts"),
            language: None,
            source_type: None,
            input_map: Some(map),
        },
    )
    .expect("fixture parses")
}

/// A one-token map resolving parse-input `(0, 0)` to `(src_line, 0)` in
/// `file`.
pub fn sample_map(
    file: &str,
    src_line: u32,
) -> SourceMap {
    let mut builder: SourceMapBuilder<'_> = SourceMapBuilder::default();

    builder.set_file(file);

    let source_id: u32 = builder.add_source_and_content(file, "const a = 1;");

    builder.add_token(0, 0, src_line, 0, Some(source_id), None);

    builder.into_owned_sourcemap().into_inner()
}

pub fn assert_same_map(
    actual: &SourceMap,
    expected: &SourceMap,
) {
    assert_eq!(actual.to_json_string(), expected.to_json_string());
}

pub fn input_value(map: SourceMap) -> SourceMapOptions {
    SourceMapOptions {
        input: Some(InputSourceMap::Mode(InputSourceMapMode::Value(map))),
        output: None,
    }
}

pub fn output_mode(mode: OutputSourceMapMode) -> SourceMapOptions {
    SourceMapOptions { input: None, output: Some(OutputSourceMap::Mode(mode)) }
}

/// `(dst_line, dst_col, src_line, src_col)` of a map's first token.
pub fn first_token(map: &SourceMap) -> (u32, u32, u32, u32) {
    let token: oxc_sourcemap::Token =
        map.get_token(0).expect("map has a first token");

    (
        token.get_dst_line(),
        token.get_dst_col(),
        token.get_src_line(),
        token.get_src_col(),
    )
}

pub fn temp_dir() -> TempDir {
    tempfile::tempdir().expect("temp dir creatable")
}

pub fn path_string(dir: &TempDir) -> String {
    dir.path().to_string_lossy().into_owned()
}

pub fn write_file(
    dir: &TempDir,
    name: &str,
    contents: &str,
) -> PathBuf {
    let path: PathBuf = dir.path().join(name);

    std::fs::write(&path, contents).expect("temp file writable");

    path
}

/// Records `"{name}:{stage}"` for every hook it is asked to run.
#[derive(Debug)]
pub struct SpyPlugin {
    name: &'static str,
    log: Arc<Mutex<Vec<String>>>,
}

impl SpyPlugin {
    pub fn new(
        name: &'static str,
        log: &Arc<Mutex<Vec<String>>>,
    ) -> Self {
        Self { name, log: Arc::clone(log) }
    }

    fn record(
        &self,
        stage: &str,
    ) {
        let mut log: MutexGuard<'_, Vec<String>> = self.log.lock().unwrap();

        log.push(format!("{}:{}", self.name, stage));
    }
}

impl Plugin for SpyPlugin {
    fn name(&self) -> Cow<'static, str> {
        self.name.into()
    }

    fn register_hook_usage(&self) -> HookUsage {
        HookUsage::Options
            | HookUsage::Start
            | HookUsage::Parse
            | HookUsage::Prepare
            | HookUsage::Transform
            | HookUsage::Finalize
            | HookUsage::Generate
            | HookUsage::End
    }

    fn options<'a>(
        &'a self,
        _ctx: &'a CommonPluginContext,
        _args: &'a OptionsArgs,
    ) -> impl Future<Output = OptionsReturn> + Send {
        async move {
            self.record("options");

            Ok(None)
        }
    }

    fn start<'a>(
        &'a self,
        _ctx: &'a PluginContext<'_>,
        _args: &'a StartArgs,
    ) -> impl Future<Output = NotifyReturn> + Send {
        async move {
            self.record("start");

            Ok(())
        }
    }

    fn parse<'a>(
        &'a self,
        _ctx: &'a PluginContext<'_>,
        _args: ParseArgs<'a>,
    ) -> impl Future<Output = ParseReturn> + Send + 'a {
        async move {
            self.record("parse");

            Ok(None)
        }
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

    fn transform<'a>(
        &'a self,
        _ctx: &'a PluginContext<'_>,
        _args: TransformArgs<'a>,
    ) -> impl Future<Output = TransformReturn> + Send + 'a {
        async move {
            self.record("transform");

            Ok(None)
        }
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

    fn generate<'a>(
        &'a self,
        _ctx: &'a PluginContext<'_>,
        _args: GenerateArgs<'a>,
    ) -> impl Future<Output = GenerateReturn> + Send + 'a {
        async move {
            self.record("generate");

            Ok(None)
        }
    }

    fn end<'a>(
        &'a self,
        _ctx: &'a PluginContext<'_>,
        _args: &'a EndArgs,
    ) -> impl Future<Output = NotifyReturn> + Send {
        async move {
            self.record("end");

            Ok(())
        }
    }
}

#[derive(Debug)]
pub struct StartObserverPlugin {
    seen: Arc<Mutex<Vec<ResolvedOptions>>>,
}

impl StartObserverPlugin {
    pub fn new(seen: &Arc<Mutex<Vec<ResolvedOptions>>>) -> Self {
        Self { seen: Arc::clone(seen) }
    }
}

impl Plugin for StartObserverPlugin {
    fn name(&self) -> Cow<'static, str> {
        "start-observer".into()
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
pub struct EndObserverPlugin {
    ended: Arc<Mutex<Vec<Option<String>>>>,
}

impl EndObserverPlugin {
    pub fn new(ended: &Arc<Mutex<Vec<Option<String>>>>) -> Self {
        Self { ended: Arc::clone(ended) }
    }
}

impl Plugin for EndObserverPlugin {
    fn name(&self) -> Cow<'static, str> {
        "end-observer".into()
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
            self.ended.lock().unwrap().push(rendered);

            Ok(())
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailStage {
    Options,
    Start,
    Prepare,
    Transform,
    Finalize,
    End,
}

#[derive(Debug)]
pub struct FailPlugin {
    name: &'static str,
    stage: FailStage,
}

impl FailPlugin {
    pub fn new(
        name: &'static str,
        stage: FailStage,
    ) -> Self {
        Self { name, stage }
    }

    fn usage(&self) -> HookUsage {
        match self.stage {
            | FailStage::Options => HookUsage::Options,
            | FailStage::Start => HookUsage::Start,
            | FailStage::Prepare => HookUsage::Prepare,
            | FailStage::Transform => HookUsage::Transform,
            | FailStage::Finalize => HookUsage::Finalize,
            | FailStage::End => HookUsage::End,
        }
    }
}

impl Plugin for FailPlugin {
    fn name(&self) -> Cow<'static, str> {
        self.name.into()
    }

    fn register_hook_usage(&self) -> HookUsage {
        self.usage()
    }

    fn options<'a>(
        &'a self,
        _ctx: &'a CommonPluginContext,
        _args: &'a OptionsArgs,
    ) -> impl Future<Output = OptionsReturn> + Send {
        async move {
            if self.stage == FailStage::Options {
                Err(anyhow::anyhow!("boom"))
            } else {
                Ok(None)
            }
        }
    }

    fn start<'a>(
        &'a self,
        _ctx: &'a PluginContext<'_>,
        _args: &'a StartArgs,
    ) -> impl Future<Output = NotifyReturn> + Send {
        async move {
            if self.stage == FailStage::Start {
                Err(anyhow::anyhow!("boom"))
            } else {
                Ok(())
            }
        }
    }

    fn prepare<'a>(
        &'a self,
        _ctx: &'a PluginContext<'_>,
        _args: PrepareArgs<'a>,
    ) -> impl Future<Output = NotifyReturn> + Send + 'a {
        async move {
            if self.stage == FailStage::Prepare {
                Err(anyhow::anyhow!("boom"))
            } else {
                Ok(())
            }
        }
    }

    fn transform<'a>(
        &'a self,
        _ctx: &'a PluginContext<'_>,
        _args: TransformArgs<'a>,
    ) -> impl Future<Output = TransformReturn> + Send + 'a {
        async move {
            if self.stage == FailStage::Transform {
                Err(anyhow::anyhow!("boom"))
            } else {
                Ok(None)
            }
        }
    }

    fn finalize<'a>(
        &'a self,
        _ctx: &'a PluginContext<'_>,
        _args: FinalizeArgs<'a>,
    ) -> impl Future<Output = NotifyReturn> + Send + 'a {
        async move {
            if self.stage == FailStage::Finalize {
                Err(anyhow::anyhow!("boom"))
            } else {
                Ok(())
            }
        }
    }

    fn end<'a>(
        &'a self,
        _ctx: &'a PluginContext<'_>,
        _args: &'a EndArgs,
    ) -> impl Future<Output = NotifyReturn> + Send {
        async move {
            if self.stage == FailStage::End {
                Err(anyhow::anyhow!("boom"))
            } else {
                Ok(())
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ParseOutcome {
    Defer,
    Win(&'static str),
    Fail,
}

#[derive(Debug)]
pub struct ParseOutcomePlugin {
    name: &'static str,
    outcome: ParseOutcome,
    log: Arc<Mutex<Vec<String>>>,
}

impl ParseOutcomePlugin {
    pub fn new(
        name: &'static str,
        outcome: ParseOutcome,
        log: &Arc<Mutex<Vec<String>>>,
    ) -> Self {
        Self { name, outcome, log: Arc::clone(log) }
    }
}

impl Plugin for ParseOutcomePlugin {
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
    ) -> impl Future<Output = ParseReturn> + Send + 'a {
        async move {
            self.log.lock().unwrap().push(self.name.to_string());

            match self.outcome {
                | ParseOutcome::Defer => Ok(None),
                | ParseOutcome::Win(code) => {
                    let ast: Ast = parse_fixture(code)?;

                    Ok(Some(AstResult { ast }))
                },
                | ParseOutcome::Fail => Err(anyhow::anyhow!("boom")),
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum GenerateOutcome {
    Defer,
    Win(&'static str),
    Fail,
}

#[derive(Debug)]
pub struct GenerateOutcomePlugin {
    name: &'static str,
    outcome: GenerateOutcome,
    log: Arc<Mutex<Vec<String>>>,
}

impl GenerateOutcomePlugin {
    pub fn new(
        name: &'static str,
        outcome: GenerateOutcome,
        log: &Arc<Mutex<Vec<String>>>,
    ) -> Self {
        Self { name, outcome, log: Arc::clone(log) }
    }
}

impl Plugin for GenerateOutcomePlugin {
    fn name(&self) -> Cow<'static, str> {
        self.name.into()
    }

    fn register_hook_usage(&self) -> HookUsage {
        HookUsage::Generate
    }

    fn generate<'a>(
        &'a self,
        _ctx: &'a PluginContext<'_>,
        _args: GenerateArgs<'a>,
    ) -> impl Future<Output = GenerateReturn> + Send + 'a {
        async move {
            self.log.lock().unwrap().push(self.name.to_string());

            match self.outcome {
                | GenerateOutcome::Defer => Ok(None),
                | GenerateOutcome::Win(code) => {
                    Ok(Some(CodeResult { code: code.to_string(), map: None }))
                },
                | GenerateOutcome::Fail => Err(anyhow::anyhow!("boom")),
            }
        }
    }
}

#[derive(Debug)]
pub struct ReplaceAstPlugin {
    pub code: &'static str,
}

impl Plugin for ReplaceAstPlugin {
    fn name(&self) -> Cow<'static, str> {
        "replace-ast".into()
    }

    fn register_hook_usage(&self) -> HookUsage {
        HookUsage::Transform
    }

    fn transform<'a>(
        &'a self,
        _ctx: &'a PluginContext<'_>,
        _args: TransformArgs<'a>,
    ) -> impl Future<Output = TransformReturn> + Send + 'a {
        async move {
            let ast: Ast = parse_fixture(self.code)?;

            Ok(Some(TransformOutput { ast }))
        }
    }
}
