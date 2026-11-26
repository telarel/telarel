use std::any::Any;
use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::Sender;

use napi::bindgen_prelude::{
    Either, Object, ObjectRef, Promise, ToNapiValue, Undefined, Unknown,
};
use napi::sys;
use napi::threadsafe_function::ThreadsafeFunctionCallMode;
use napi::{Env, Result, Status};
use oxc::span::SourceType;

use telarel_common::{
    Ast, AstResult, CodeResult, HookUsage, Language, ResolvableOptions,
    SourceMap,
};
use telarel_plugin::__internal::{
    HookFuture, SharedTransformBatch, TransformBatch, TransformBatchable,
};
use telarel_plugin::{
    CommonPluginContext, EndArgs, FinalizeArgs, GenerateArgs, GenerateReturn,
    NotifyReturn, OptionsArgs, ParseArgs, ParseReturn, PluginContext,
    PluginError, PluginHookMeta, Pluginable, PrepareArgs, StartArgs,
    TransformArgs, TransformOutput, TransformReturn,
};

use crate::_types::ast::JsAst;
use crate::_types::options::{
    generator_options_to_js, language_label, parse_generator_options,
    parse_parser_options, parser_options_to_js, source_type_label,
};
use crate::_types::path::path_label;
use crate::_types::plugin::hooks::{
    JsCodeOutput, JsGeneratorHookOptions, JsOptionsOutput, JsParseOutput,
    JsParserHookOptions, JsResolvedOptions, JsTransformOutput,
    optional_output_options_to_js, optional_source_map_options_to_js,
};
use crate::_types::plugin::order::read_hook_meta;
use crate::_types::refs::{RefList, SharedRef, SharedStr};
use crate::_types::sourcemap::JsSourceMap;
use crate::ast::estree::{ast_from_json, ast_to_estree_json};
use crate::ast::normalized::js_to_ast;
use crate::ast::resolve::{source_type_for_generate, source_type_for_options};
use crate::ast::sourcemap::{js_source_map_to_owned, source_map_to_js};
use crate::comment;
use crate::errors::plugin::decode_plugin_error;
use crate::plugin::build::NAME_REQUIRED;
use crate::plugin::build::TRANSFORM_BATCH_KEY;
use crate::plugin::build::to_patch_plugins;
use crate::plugin::hooks::{
    CtxTsfn, EndCall, EndTsfn, FinalizeTsfn, GenerateCall, GenerateTsfn,
    HookCall, ObserveCall, OptionsCall, OptionsTsfn, ParseCall, ParseTsfn,
    PrepareTsfn, StartCall, StartTsfn, TransformBatchCall, TransformBatchTsfn,
    TransformCall, TransformTsfn, hook_scan, scan_hook,
};
use crate::plugin::options::{parse_language, parse_source_type};

/// A JS plugin object bridged into the Rust plugin layer.
///
/// The bridge scans every hook TSFN on the object at bridge time and keeps it
/// on the adapter. A late plugin, wrapped during the `options` fixpoint,
/// therefore keeps its own hook functions independent of the entry plugins.
pub struct JsPlugin {
    name: String,
    tsfn_options: Option<OptionsTsfn>,
    tsfn_start: Option<StartTsfn>,
    tsfn_parse: Option<ParseTsfn>,
    tsfn_prepare: Option<PrepareTsfn>,
    tsfn_transform: Option<TransformTsfn>,
    tsfn_transform_batch: Option<Arc<TransformBatchTsfn>>,
    tsfn_finalize: Option<FinalizeTsfn>,
    tsfn_generate: Option<GenerateTsfn>,
    tsfn_end: Option<EndTsfn>,
    hook_meta_options: Option<PluginHookMeta>,
    hook_meta_start: Option<PluginHookMeta>,
    hook_meta_parse: Option<PluginHookMeta>,
    hook_meta_prepare: Option<PluginHookMeta>,
    hook_meta_transform: Option<PluginHookMeta>,
    hook_meta_finalize: Option<PluginHookMeta>,
    hook_meta_generate: Option<PluginHookMeta>,
    hook_meta_end: Option<PluginHookMeta>,
    /// The dynamic release list of the compile, shared with every JS plugin
    /// and with the compile task's `finally`.
    refs: Arc<RefList>,
    /// The rooted raw descriptor. A transform batch payload carries it back to
    /// JS so the batch runner can read the descriptor's internal entry.
    plugin_ref: SharedRef,
}

impl JsPlugin {
    /// Bridge a raw JS plugin object into a JS plugin adapter. Returns
    /// `InvalidArg` when `name` is absent. `plugin_ref` is the caller's rooted
    /// descriptor. The adapter keeps a clone for batch payloads.
    pub fn from_object(
        object: &Object<'static>,
        refs: &Arc<RefList>,
        plugin_ref: &SharedRef,
    ) -> Result<Self> {
        let name: String = object.get::<String>("name")?.ok_or_else(|| {
            napi::Error::new(Status::InvalidArg, NAME_REQUIRED.to_owned())
        })?;

        let (
            tsfn_options,
            tsfn_start,
            tsfn_parse,
            tsfn_prepare,
            tsfn_transform,
            tsfn_finalize,
            tsfn_generate,
            tsfn_end,
        ) = hook_scan!(object);

        let tsfn_transform_batch: Option<Arc<TransformBatchTsfn>> =
            scan_hook::<TransformBatchCall>(object, TRANSFORM_BATCH_KEY)?
                .map(Arc::new);

        let hook_meta_options: Option<PluginHookMeta> =
            read_hook_meta(object, "options")?;

        let hook_meta_start: Option<PluginHookMeta> =
            read_hook_meta(object, "start")?;

        let hook_meta_parse: Option<PluginHookMeta> =
            read_hook_meta(object, "parse")?;

        let hook_meta_prepare: Option<PluginHookMeta> =
            read_hook_meta(object, "prepare")?;

        let hook_meta_transform: Option<PluginHookMeta> =
            read_hook_meta(object, "transform")?;

        let hook_meta_finalize: Option<PluginHookMeta> =
            read_hook_meta(object, "finalize")?;

        let hook_meta_generate: Option<PluginHookMeta> =
            read_hook_meta(object, "generate")?;

        let hook_meta_end: Option<PluginHookMeta> =
            read_hook_meta(object, "end")?;

        Ok(Self {
            name,
            tsfn_options,
            tsfn_start,
            tsfn_parse,
            tsfn_prepare,
            tsfn_transform,
            tsfn_transform_batch,
            tsfn_finalize,
            tsfn_generate,
            tsfn_end,
            hook_meta_options,
            hook_meta_start,
            hook_meta_parse,
            hook_meta_prepare,
            hook_meta_transform,
            hook_meta_finalize,
            hook_meta_generate,
            hook_meta_end,
            refs: Arc::clone(refs),
            plugin_ref: plugin_ref.clone(),
        })
    }
}

/// Convert a napi error that a hook call raised into a plugin error.
///
/// A JS `PluginError` travels as a private envelope in the message. Ordinary
/// throwables keep their existing `napi::Error` rendering.
fn hook_error(error: napi::Error) -> PluginError {
    match decode_plugin_error(&error.reason) {
        | Some(decoded) => decoded,
        | None => PluginError::new(error.to_string()),
    }
}

/// Convert a read-back failure into a plugin error. The plugin crate has no
/// `anyhow` conversion impl.
fn anyhow_to_plugin_error(error: anyhow::Error) -> PluginError {
    PluginError::new(format!("{error:#}"))
}

fn ast_hook_parts(ast: &Ast) -> (String, String, Option<JsSourceMap>, String) {
    (
        ast_to_estree_json(ast),
        ast.source().to_string(),
        ast.map().map(source_map_to_js),
        ast.language().as_str().to_string(),
    )
}

/// The outbound payload fields shared by the single and batch transform calls.
struct TransformCallParts {
    cwd: SharedStr,
    file: SharedStr,
    language: SharedStr,
    source_type: SharedStr,
    ast_json: String,
    code: String,
    map: Option<JsSourceMap>,
    ast_language: String,
}

/// The input side of a transform read-back, in the form the plugin saw.
struct TransformInput {
    ast: Ast,
    ast_json: String,
    code: String,
    map: Option<JsSourceMap>,
}

impl TransformCallParts {
    /// Build the shared payload fields from the plugin context and input AST.
    ///
    /// The bridge anchors the UTF-16 map to the AST's own source, the parse
    /// input, not the module's original code. It therefore matches the byte
    /// spans that the codec emits.
    fn new(
        ctx: &PluginContext<'_>,
        ast: &Ast,
    ) -> Self {
        let (ast_json, code, map, ast_language): (
            String,
            String,
            Option<JsSourceMap>,
            String,
        ) = ast_hook_parts(ast);

        Self {
            cwd: SharedStr::new(&path_label(ctx.cwd)),
            file: SharedStr::new(ctx.module.file),
            language: SharedStr::new(language_label(Some(ctx.module.language))),
            source_type: SharedStr::new(source_type_label(Some(
                ctx.module.source_type,
            ))),
            ast_json,
            code,
            map,
            ast_language,
        }
    }

    /// Clone the shared fields into a single transform call.
    fn call(&self) -> TransformCall {
        TransformCall {
            cwd: self.cwd.clone(),
            file: self.file.clone(),
            language: self.language.clone(),
            source_type: self.source_type.clone(),
            ast_json: self.ast_json.clone(),
            code: self.code.clone(),
            map: self.map.clone(),
            ast_language: self.ast_language.clone(),
        }
    }

    /// Clone the shared fields into a batch call and add the run's descriptors.
    fn batch_call(
        &self,
        plugins: Vec<SharedRef>,
    ) -> TransformBatchCall {
        TransformBatchCall {
            cwd: self.cwd.clone(),
            file: self.file.clone(),
            language: self.language.clone(),
            source_type: self.source_type.clone(),
            ast_json: self.ast_json.clone(),
            code: self.code.clone(),
            map: self.map.clone(),
            ast_language: self.ast_language.clone(),
            plugins,
        }
    }

    /// Consume the read-back fields for the input AST that produced the call.
    fn into_input(
        self,
        ast: Ast,
    ) -> TransformInput {
        TransformInput {
            ast,
            ast_json: self.ast_json,
            code: self.code,
            map: self.map,
        }
    }
}

/// The transform read-back grammar: a returned label wins. An absent label
/// means the input AST's grammar still anchors the rebuild.
fn transform_read_back_language(
    returned: Option<&str>,
    input: Language,
) -> std::result::Result<Language, PluginError> {
    let returned: Option<Language> = match returned {
        | Some(text) => parse_language(text).map_err(hook_error)?,
        | None => None,
    };

    Ok(returned.unwrap_or(input))
}

/// Call one transform TSFN and unmarshal its sync or async return.
async fn call_transform_tsfn<C>(
    tsfn: &CtxTsfn<C>,
    call: C,
) -> std::result::Result<Option<JsTransformOutput>, PluginError>
where
    C: HookCall<Return = Option<JsTransformOutput>>,
{
    let output: Either<
        Promise<Option<JsTransformOutput>>,
        Option<JsTransformOutput>,
    > = tsfn.call_async_catch(call).await.map_err(hook_error)?;

    match output {
        | Either::A(promise) => promise.await.map_err(hook_error),
        | Either::B(value) => Ok(value),
    }
}

/// Apply the single and batch transform read-back rules to one JS return.
///
/// An unchanged return short-circuits to `None` unless comments need
/// re-anchoring, in which case the sanitized input AST returns as `Some`. Any
/// changed field rebuilds a fresh owned AST from the returned JSON, code, map,
/// and effective grammar. Both transform paths must use this helper so their
/// pass-through and rebuild behavior cannot drift.
fn finish_transform_output(
    replaced: JsTransformOutput,
    input: TransformInput,
) -> std::result::Result<Option<TransformOutput>, PluginError> {
    let language: Language = transform_read_back_language(
        replaced.language.as_deref(),
        input.ast.language(),
    )?;

    if replaced.ast_json == input.ast_json
        && replaced.code == input.code
        && replaced.map == input.map
        && language == input.ast.language()
    {
        let mut ast: Ast = input.ast;

        if comment::reanchor_ast(&mut ast) {
            return Ok(Some(TransformOutput { ast }));
        }

        return Ok(None);
    }

    // The returned program's `sourceType` owns the module kind. The chosen
    // grammar only supplies the unambiguous parser input.
    let source_type: SourceType = source_type_for_generate(language);

    let input_map: Option<SourceMap> = match replaced.map {
        | Some(map) => {
            Some(js_source_map_to_owned(&map).map_err(anyhow_to_plugin_error)?)
        },
        | None => None,
    };

    let ast: Ast = ast_from_json(
        &replaced.ast_json,
        Arc::from(replaced.code),
        input_map,
        source_type,
    )
    .map_err(anyhow_to_plugin_error)?;

    Ok(Some(TransformOutput { ast }))
}

/// Apply the batch read-back rules to one unmarshalled batch return.
///
/// `None` means the batch accepted no member change. A final output equal to
/// the batch input also reads as `None`, even when an earlier member changed
/// the AST inside the batch. The driver keeps any earlier accepted output and
/// its `changed` state.
fn finish_transform_batch_output(
    replaced: Option<JsTransformOutput>,
    input: TransformInput,
) -> std::result::Result<Option<TransformOutput>, PluginError> {
    match replaced {
        | Some(replaced) => finish_transform_output(replaced, input),
        | None => Ok(None),
    }
}

/// Holds a returned options bag's rooted plugin descriptors until
/// `to_patch_plugins` adopts them.
///
/// The descriptors arrive already rooted. `ObjectRef`'s `Drop` is a no-op, so
/// an unmarshal error before `to_patch_plugins` would leak every root. The guard
/// moves them onto the release list on drop, and `FunctionTask::finally`
/// drains that list when the compile aborts.
struct PendingPluginRoots<'a> {
    descriptors: Option<Vec<ObjectRef<false>>>,
    refs: &'a Arc<RefList>,
}

impl PendingPluginRoots<'_> {
    fn take(&mut self) -> Vec<ObjectRef<false>> {
        self.descriptors.take().unwrap_or_default()
    }
}

impl Drop for PendingPluginRoots<'_> {
    fn drop(&mut self) {
        let Some(descriptors) = self.descriptors.take() else {
            return;
        };

        for descriptor in descriptors {
            self.refs.push_ref(SharedRef::from_ref(descriptor));
        }
    }
}

/// Unmarshal an `options` hook return into the pipeline bag on the JS thread,
/// where the returned plugin descriptors are reachable.
///
/// `null` or `undefined` means no change. A returned object is a patch: it
/// carries only the fields the hook changed and the descriptors it added.
/// Rooted descriptors join the compile's release list.
fn unmarshal_options(
    output: Option<JsOptionsOutput>,
    env: &Env,
    refs: &Arc<RefList>,
) -> std::result::Result<Option<OptionsArgs>, PluginError> {
    let Some(output) = output else {
        return Ok(None);
    };

    // The `Option<JsOptionsOutput>` conversion (`ObjectRef`'s `FromNapiValue`)
    // created one `napi_create_reference` root per descriptor, owned by
    // `ctx.value` in the resolution callback. These roots must move onto the
    // release list as-is. Re-rooting them with `SharedRef::new` (`create_ref`)
    // would leak one `napi_ref` per descriptor per options call.
    // `to_patch_plugins` wraps the same roots. The entry bridge in `compile`
    // does too. The guard keeps every early error path from dropping them
    // unrooted.
    let mut pending_plugins: PendingPluginRoots<'_> =
        PendingPluginRoots { descriptors: output.plugins, refs };

    // Parse the scalar fields before touching the descriptor table. The
    // returned bag is a patch, and `to_patch_plugins` updates the descriptor
    // view only after every fallible step succeeds. On an error the compile
    // aborts. `pending_plugins` moves the roots onto the release list for
    // `FunctionTask::finally`.
    let language: Option<telarel_common::Language> = match &output.language {
        | Some(text) => parse_language(text).map_err(hook_error)?,
        | None => None,
    };

    let source_type: Option<telarel_common::SourceType> =
        match &output.source_type {
            | Some(text) => parse_source_type(text).map_err(hook_error)?,
            | None => None,
        };

    let output_options: Option<telarel_common::OutputOptions> =
        crate::_types::options::parse_output_options(output.output.as_ref())
            .map_err(hook_error)?;

    let source_map: Option<telarel_common::SourceMapOptions> =
        match &output.source_map {
            | Some(value) => Some(
                crate::_types::options::parse_source_map_options(value)
                    .map_err(hook_error)?,
            ),
            | None => None,
        };

    let parser: Option<telarel_common::ParserOptions> = match &output.parser {
        | Some(value) => Some(parse_parser_options(value).map_err(hook_error)?),
        | None => None,
    };

    let generator: Option<telarel_common::GeneratorOptions> =
        match &output.generator {
            | Some(value) => {
                Some(parse_generator_options(value).map_err(hook_error)?)
            },
            | None => None,
        };

    // The wrapper returns only the descriptors the hook added, so this list
    // carries the patch's new plugins. An omitted field adds nothing. The
    // pipeline appends these to the settled list and skips instances that are
    // already present. `to_patch_plugins` bridges the patch descriptors and
    // appends them to the options-payload view, so later hooks see the full
    // settled list. On failure it releases the rooted references and leaves
    // the view untouched.
    let plugins: Vec<telarel_plugin::SharedPluginable> =
        to_patch_plugins(env, pending_plugins.take(), refs)
            .map_err(hook_error)?;

    let args: OptionsArgs = OptionsArgs {
        options: ResolvableOptions {
            cwd: output.cwd.map(PathBuf::from),
            file: output.file,
            language,
            source_type,
            parser,
            generator,
            output: output_options,
            source_map,
        },
        plugins,
    };

    Ok(Some(args))
}

/// The raw return value of the TSFN that bridges an `options` hook.
///
/// The raw `options` hook is an async function that returns
/// `Promise<Option<JsOptionsOutput>>`. The TSFN delivers the raw promise
/// object to the JS-thread callback, and the callback wires the resolution
/// bridging.
pub type OptionsReturn = Unknown<'static>;

/// Bridge a promise's resolution on the JS thread.
///
/// Every failure settles the outcome channel first, because an escaping napi
/// error here aborts the process.
fn await_options_promise(
    value: Unknown,
    env: Env,
    refs: Arc<RefList>,
    sender: Sender<std::result::Result<Option<OptionsArgs>, PluginError>>,
) -> napi::Result<()> {
    use napi::bindgen_prelude::{CallbackContext, JsValue, PromiseRaw};

    // The conversion is the JS-value passthrough (infallible in practice). A
    // failure must still settle the channel. See this function's docs.
    let raw_value: sys::napi_value = match unsafe {
        <Unknown as ToNapiValue>::to_napi_value(env.raw(), value)
    } {
        | Ok(raw_value) => raw_value,
        | Err(error) => {
            let _ = sender.send(Err(hook_error(error)));

            return Ok(());
        },
    };

    // Make sure that the value is a promise before wiring. `PromiseRaw::new`
    // performs no promise check.
    let is_promise: bool = value.is_promise().unwrap_or(false);

    if !is_promise {
        let _ = sender.send(Err(PluginError::new(
            "`options` hook must return a Promise (an async function), \
             got a non-promise value",
        )));

        return Ok(());
    }

    let raw: PromiseRaw<'_, Option<JsOptionsOutput>> =
        PromiseRaw::new(env.raw(), raw_value);

    let resolve_sender = sender.clone();

    let reject_sender = sender.clone();

    let resolve = move |ctx: CallbackContext<Option<JsOptionsOutput>>| {
        let outcome: std::result::Result<Option<OptionsArgs>, PluginError> =
            unmarshal_options(ctx.value, &ctx.env, &refs);

        let _ = resolve_sender.send(outcome);

        Ok(())
    };

    let reject = move |ctx: CallbackContext<Unknown>| {
        let error: napi::Error =
            napi::Error::from_unknown_without_coercion(ctx.value);

        let _ = reject_sender.send(Err(hook_error(error)));

        Ok(())
    };

    // `then`/`catch` failures leave the worker's `recv` hanging. When they
    // surface from this callback, they become a fatal exception. Error the
    // channel and swallow the napi error instead.
    if let Err(error) = raw.then(resolve).and_then(|wired| wired.catch(reject))
    {
        let _ = sender.send(Err(hook_error(error)));
    }

    Ok(())
}

/// The `options` hook call.
///
/// It resolves on the JS thread because the bridge must wrap a returned bag's
/// plugin descriptors where JS objects are reachable. The outcome travels
/// back to the worker through a channel.
async fn call_options_tsfn(
    tsfn: &OptionsTsfn,
    call: OptionsCall,
    refs: &Arc<RefList>,
) -> std::result::Result<Option<OptionsArgs>, PluginError> {
    type OptionsOutcome = std::result::Result<Option<OptionsArgs>, PluginError>;

    let (sender, receiver): (Sender<OptionsOutcome>, _) =
        std::sync::mpsc::channel();

    // The closure outlives this call, because it runs when the promise
    // resolves. The shared list therefore moves in as an owned handle.
    let refs: Arc<RefList> = Arc::clone(refs);

    let status: napi::Status = tsfn.call_with_return_value(
        call,
        ThreadsafeFunctionCallMode::NonBlocking,
        move |output: napi::Result<OptionsReturn>, env: Env| {
            match output {
                | Err(error) => {
                    // The promise was never created, because a synchronous
                    // throw inside the raw hook surfaced as a pending
                    // exception. Surface it as the hook error.
                    let _ = sender.send(Err(hook_error(error)));

                    Ok(())
                },
                | Ok(value) => await_options_promise(value, env, refs, sender),
            }
        },
    );

    if status == napi::Status::Closing {
        return Err(PluginError::new(
            "options hook: threadsafe function closed",
        ));
    }

    // The outcome arrives through the promise machinery on the JS thread. A
    // plain blocking receive on the worker thread is safe. The single-task
    // worker's runtime is parked inside `block_on`, and the JS thread is free
    // to drive the promise to resolution.
    std::sync::mpsc::Receiver::recv(&receiver).map_err(|_| {
        PluginError::new("options hook: callback channel closed")
    })?
}

/// Build the `options` hook payload from the current options bag.
///
/// An absent stage group stays `None`, so Telarel omits it from the payload.
fn options_call(
    options: &ResolvableOptions,
    plugins: Vec<SharedRef>,
) -> OptionsCall {
    OptionsCall {
        cwd: options.cwd.as_deref().map(|cwd| SharedStr::new(&path_label(cwd))),
        file: options.file.as_deref().map(SharedStr::new),
        language: SharedStr::new(language_label(options.language)),
        source_type: SharedStr::new(source_type_label(options.source_type)),
        parser: options.parser.as_ref().map(parser_options_to_js),
        generator: options.generator.as_ref().map(generator_options_to_js),
        output: optional_output_options_to_js(options.output.as_ref()),
        source_map: optional_source_map_options_to_js(
            options.source_map.as_ref(),
        ),
        plugins,
    }
}

/// The transform hooks that a JS plugin carries.
struct TransformHookSet {
    transform: bool,
    batch: bool,
}

impl Pluginable for JsPlugin {
    fn call_name(&self) -> Cow<'static, str> {
        Cow::Owned(self.name.clone())
    }

    fn call_register_hook_usage(&self) -> HookUsage {
        let mut usage: HookUsage = HookUsage::default();

        if self.tsfn_options.is_some() {
            usage.insert(HookUsage::Options);
        }

        if self.tsfn_start.is_some() {
            usage.insert(HookUsage::Start);
        }

        if self.tsfn_parse.is_some() {
            usage.insert(HookUsage::Parse);
        }

        if self.tsfn_prepare.is_some() {
            usage.insert(HookUsage::Prepare);
        }

        if self.tsfn_transform.is_some() {
            usage.insert(HookUsage::Transform);
        }

        if self.tsfn_finalize.is_some() {
            usage.insert(HookUsage::Finalize);
        }

        if self.tsfn_generate.is_some() {
            usage.insert(HookUsage::Generate);
        }

        if self.tsfn_end.is_some() {
            usage.insert(HookUsage::End);
        }

        usage
    }

    fn call_options<'a>(
        &'a self,
        _ctx: &'a CommonPluginContext,
        args: &'a OptionsArgs,
    ) -> HookFuture<'a, telarel_plugin::OptionsReturn> {
        Box::pin(async move {
            let Some(tsfn) = self.tsfn_options.as_ref() else {
                return Ok(None);
            };

            let call: OptionsCall =
                options_call(&args.options, self.refs.bag_snapshot());

            call_options_tsfn(tsfn, call, &self.refs).await
        })
    }

    fn call_options_meta(&self) -> Option<PluginHookMeta> {
        self.hook_meta_options
    }

    fn call_start<'a>(
        &'a self,
        _ctx: &'a PluginContext<'_>,
        args: &'a StartArgs,
    ) -> HookFuture<'a, NotifyReturn> {
        Box::pin(async move {
            let Some(tsfn) = self.tsfn_start.as_ref() else {
                return Ok(());
            };

            let call: StartCall = StartCall {
                cwd: SharedStr::new(&path_label(&args.options.cwd)),
                file: SharedStr::new(&args.options.file),
                language: SharedStr::new(language_label(Some(
                    args.options.language,
                ))),
                source_type: SharedStr::new(source_type_label(Some(
                    args.options.source_type,
                ))),
                options: JsResolvedOptions::from(&args.options),
            };

            let output: Either<Promise<Undefined>, Undefined> =
                tsfn.call_async_catch(call).await.map_err(hook_error)?;

            if let Either::A(promise) = output {
                promise.await.map_err(hook_error)?;
            }

            Ok(())
        })
    }

    fn call_start_meta(&self) -> Option<PluginHookMeta> {
        self.hook_meta_start
    }

    fn call_parse<'a>(
        &'a self,
        ctx: &'a PluginContext<'_>,
        args: ParseArgs<'a>,
    ) -> HookFuture<'a, ParseReturn> {
        Box::pin(async move {
            let Some(tsfn) = self.tsfn_parse.as_ref() else {
                return Ok(None);
            };

            let call: ParseCall = ParseCall {
                cwd: SharedStr::new(&path_label(ctx.cwd)),
                file: SharedStr::new(ctx.module.file),
                code: args.code.to_owned(),
                language: SharedStr::new(language_label(Some(
                    ctx.module.language,
                ))),
                source_type: SharedStr::new(source_type_label(Some(
                    ctx.module.source_type,
                ))),
                options: JsParserHookOptions::from(args.options),
            };

            let output: Either<
                Promise<Option<JsParseOutput>>,
                Option<JsParseOutput>,
            > = tsfn.call_async_catch(call).await.map_err(hook_error)?;

            let replaced: Option<JsParseOutput> = match output {
                | Either::A(promise) => promise.await.map_err(hook_error)?,
                | Either::B(value) => value,
            };

            let Some(replaced) = replaced else {
                return Ok(None);
            };

            // Parser options own the grammar. The bridge deliberately ignores
            // a returned AST's language label, because it must read the spans
            // under the grammar that the builtin parser resolved.
            let source_type: SourceType = source_type_for_options(
                &args.options.file,
                Some(args.options.language),
                args.options.source_type,
            );

            let ast: Ast = js_to_ast(replaced.ast, source_type)
                .map_err(anyhow_to_plugin_error)?;

            Ok(Some(AstResult { ast }))
        })
    }

    fn call_parse_meta(&self) -> Option<PluginHookMeta> {
        self.hook_meta_parse
    }

    fn call_prepare<'a>(
        &'a self,
        ctx: &'a PluginContext<'_>,
        args: PrepareArgs<'a>,
    ) -> HookFuture<'a, NotifyReturn> {
        Box::pin(async move {
            let Some(tsfn) = self.tsfn_prepare.as_ref() else {
                return Ok(());
            };

            let (ast_json, code, map, ast_language): (
                String,
                String,
                Option<JsSourceMap>,
                String,
            ) = ast_hook_parts(args.ast);

            let call: ObserveCall = ObserveCall {
                cwd: SharedStr::new(&path_label(ctx.cwd)),
                file: SharedStr::new(ctx.module.file),
                language: SharedStr::new(language_label(Some(
                    ctx.module.language,
                ))),
                source_type: SharedStr::new(source_type_label(Some(
                    ctx.module.source_type,
                ))),
                ast_json,
                code,
                map,
                ast_language,
            };

            let output: Either<Promise<Undefined>, Undefined> =
                tsfn.call_async_catch(call).await.map_err(hook_error)?;

            if let Either::A(promise) = output {
                promise.await.map_err(hook_error)?;
            }

            Ok(())
        })
    }

    fn call_prepare_meta(&self) -> Option<PluginHookMeta> {
        self.hook_meta_prepare
    }

    fn call_transform<'a>(
        &'a self,
        ctx: &'a PluginContext<'_>,
        args: TransformArgs,
    ) -> HookFuture<'a, TransformReturn> {
        Box::pin(async move {
            let Some(tsfn) = self.tsfn_transform.as_ref() else {
                return Ok(None);
            };

            let parts: TransformCallParts =
                TransformCallParts::new(ctx, &args.ast);

            let replaced: Option<JsTransformOutput> =
                call_transform_tsfn(tsfn, parts.call()).await?;

            let Some(replaced) = replaced else {
                return Ok(None);
            };

            finish_transform_output(replaced, parts.into_input(args.ast))
        })
    }

    fn call_transform_meta(&self) -> Option<PluginHookMeta> {
        self.hook_meta_transform
    }

    fn as_transform_batchable(&self) -> Option<&dyn TransformBatchable> {
        let hooks: TransformHookSet = TransformHookSet {
            transform: self.tsfn_transform.is_some(),
            batch: self.tsfn_transform_batch.is_some(),
        };

        if hooks.is_batchable() { Some(self) } else { None }
    }

    fn call_finalize<'a>(
        &'a self,
        ctx: &'a PluginContext<'_>,
        args: FinalizeArgs<'a>,
    ) -> HookFuture<'a, NotifyReturn> {
        Box::pin(async move {
            let Some(tsfn) = self.tsfn_finalize.as_ref() else {
                return Ok(());
            };

            let (ast_json, code, map, ast_language): (
                String,
                String,
                Option<JsSourceMap>,
                String,
            ) = ast_hook_parts(args.ast);

            let call: ObserveCall = ObserveCall {
                cwd: SharedStr::new(&path_label(ctx.cwd)),
                file: SharedStr::new(ctx.module.file),
                language: SharedStr::new(language_label(Some(
                    ctx.module.language,
                ))),
                source_type: SharedStr::new(source_type_label(Some(
                    ctx.module.source_type,
                ))),
                ast_json,
                code,
                map,
                ast_language,
            };

            let output: Either<Promise<Undefined>, Undefined> =
                tsfn.call_async_catch(call).await.map_err(hook_error)?;

            if let Either::A(promise) = output {
                promise.await.map_err(hook_error)?;
            }

            Ok(())
        })
    }

    fn call_finalize_meta(&self) -> Option<PluginHookMeta> {
        self.hook_meta_finalize
    }

    fn call_generate<'a>(
        &'a self,
        ctx: &'a PluginContext<'_>,
        args: GenerateArgs<'a>,
    ) -> HookFuture<'a, GenerateReturn> {
        Box::pin(async move {
            let Some(tsfn) = self.tsfn_generate.as_ref() else {
                return Ok(None);
            };

            let call: GenerateCall = GenerateCall {
                cwd: SharedStr::new(&path_label(ctx.cwd)),
                file: SharedStr::new(ctx.module.file),
                language: SharedStr::new(language_label(Some(
                    ctx.module.language,
                ))),
                source_type: SharedStr::new(source_type_label(Some(
                    ctx.module.source_type,
                ))),
                ast: JsAst::from_ast(args.ast)
                    .map_err(anyhow_to_plugin_error)?,
                options: JsGeneratorHookOptions::from(args.options),
            };

            let output: Either<
                Promise<Option<JsCodeOutput>>,
                Option<JsCodeOutput>,
            > = tsfn.call_async_catch(call).await.map_err(hook_error)?;

            let replaced: Option<JsCodeOutput> = match output {
                | Either::A(promise) => promise.await.map_err(hook_error)?,
                | Either::B(value) => value,
            };

            let Some(replaced) = replaced else {
                return Ok(None);
            };

            let map: Option<SourceMap> = match replaced.map {
                | Some(map) => Some(
                    js_source_map_to_owned(&map)
                        .map_err(anyhow_to_plugin_error)?,
                ),
                | None => None,
            };

            Ok(Some(CodeResult { code: replaced.code, map }))
        })
    }

    fn call_generate_meta(&self) -> Option<PluginHookMeta> {
        self.hook_meta_generate
    }

    fn call_end<'a>(
        &'a self,
        ctx: &'a PluginContext<'_>,
        args: &'a EndArgs,
    ) -> HookFuture<'a, NotifyReturn> {
        Box::pin(async move {
            let Some(tsfn) = self.tsfn_end.as_ref() else {
                return Ok(());
            };

            let call: EndCall = EndCall {
                cwd: SharedStr::new(&path_label(ctx.cwd)),
                file: SharedStr::new(ctx.module.file),
                language: SharedStr::new(language_label(Some(
                    ctx.module.language,
                ))),
                source_type: SharedStr::new(source_type_label(Some(
                    ctx.module.source_type,
                ))),
                error: args.error.as_ref().map(ToString::to_string),
            };

            let output: Either<Promise<Undefined>, Undefined> =
                tsfn.call_async_catch(call).await.map_err(hook_error)?;

            if let Either::A(promise) = output {
                promise.await.map_err(hook_error)?;
            }

            Ok(())
        })
    }

    fn call_end_meta(&self) -> Option<PluginHookMeta> {
        self.hook_meta_end
    }
}

impl TransformHookSet {
    /// A plugin is batchable only when it has both the single transform hook
    /// and the internal batch runner. Either missing keeps the single path.
    fn is_batchable(&self) -> bool {
        self.transform && self.batch
    }
}

/// One JS batch call over the raw descriptors of a contiguous transform run.
struct JsTransformBatch {
    tsfn: Arc<TransformBatchTsfn>,
    plugins: Vec<SharedRef>,
}

impl TransformBatchable for JsPlugin {
    fn make_transform_batch(
        &self,
        members: &[&dyn Pluginable],
    ) -> Option<SharedTransformBatch> {
        let tsfn: Arc<TransformBatchTsfn> =
            Arc::clone(self.tsfn_transform_batch.as_ref()?);

        let mut plugins: Vec<SharedRef> = Vec::with_capacity(members.len());

        for member in members {
            let js: &JsPlugin =
                (*member as &dyn Any).downcast_ref::<JsPlugin>()?;

            plugins.push(js.plugin_ref.clone());
        }

        Some(Arc::new(JsTransformBatch { tsfn, plugins }))
    }
}

impl TransformBatch for JsTransformBatch {
    fn call_transform<'a>(
        &'a self,
        ctx: &'a PluginContext<'_>,
        args: TransformArgs,
    ) -> HookFuture<'a, TransformReturn> {
        Box::pin(async move {
            let parts: TransformCallParts =
                TransformCallParts::new(ctx, &args.ast);

            let replaced: Option<JsTransformOutput> = call_transform_tsfn(
                &self.tsfn,
                parts.batch_call(self.plugins.clone()),
            )
            .await?;

            finish_transform_batch_output(replaced, parts.into_input(args.ast))
        })
    }
}

/// Tests for the env-free parts of the hook marshaling: the options
/// unmarshalling, the transform read-back rules, and the bag and release-list
/// bookkeeping.
///
/// The tests cover the options patch and the omitted-`plugins` settled view.
/// The ESTree serialization and map parsing live in `crate::ast`,
/// where tests cover them. Only a JS runtime can observe the descriptor-loop and
/// promise-wiring paths, because they touch napi FFI (reference rooting,
/// `napi_is_promise`). The options-injection tests in
/// `tests/telarel/test/compile.test.ts` exercise that surface.
#[cfg(test)]
mod tests;
