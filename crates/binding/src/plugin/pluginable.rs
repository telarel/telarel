use std::borrow::Cow;
use std::sync::Arc;
use std::sync::mpsc::Sender;

use napi::bindgen_prelude::{
    Either, Object, Promise, ToNapiValue, Undefined, Unknown,
};
use napi::sys;
use napi::threadsafe_function::ThreadsafeFunctionCallMode;
use napi::{Env, Result, Status};
use oxc::span::SourceType;

use telarel_common::{
    Ast, AstResult, CodeResult, HookUsage, ResolvableOptions, SourceMap,
};
use telarel_plugin::__internal::HookFuture;
use telarel_plugin::{
    CommonPluginContext, EndArgs, FinalizeArgs, GenerateArgs, GenerateReturn,
    NotifyReturn, OptionsArgs, ParseArgs, ParseReturn, PluginContext,
    PluginHookMeta, Pluginable, PrepareArgs, StartArgs, TransformArgs,
    TransformOutput, TransformReturn,
};

use crate::_types::plugin::hooks::{
    JsAstOutput, JsCodeOutput, JsGeneratorOptions, JsOptionsOutput,
    JsParseOutput, JsParserOptions, JsTransformOutput,
};
use crate::_types::plugin::order::read_hook_meta;
use crate::plugin::build::NAME_REQUIRED;
use crate::plugin::build::to_plugins;
use crate::plugin::hooks::{
    EndCall, EndTsfn, FinalizeTsfn, GenerateCall, GenerateTsfn, ObserveCall,
    OptionsCall, OptionsTsfn, ParseCall, ParseTsfn, PrepareTsfn, RefList,
    SharedStr, StartCall, StartTsfn, TransformCall, TransformTsfn, hook_scan,
};
use crate::plugin::options::{
    language_label, parse_language, parse_source_type, source_type_label,
};
use crate::plugin::spans::{SpanMap, spans_to_bytes, spans_to_utf16};

/// A JS plugin object bridged into the Rust plugin layer.
///
/// Every hook TSFN the object carries is scanned at bridge time and kept on
/// the adapter itself, so late plugins (wrapped during the `options`
/// fixpoint) keep their own hook functions independent of the entry plugins.
pub struct JsPlugin {
    name: String,
    tsfn_options: Option<OptionsTsfn>,
    tsfn_start: Option<StartTsfn>,
    tsfn_parse: Option<ParseTsfn>,
    tsfn_prepare: Option<PrepareTsfn>,
    tsfn_transform: Option<TransformTsfn>,
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
    /// The compile's dynamic release list, shared with every JS plugin and
    /// with the compile task's `finally`.
    refs: Arc<RefList>,
}

impl JsPlugin {
    /// Bridge a raw JS plugin object: validate `name` and scan the hook
    /// slots, wrapping present hook functions into TSFNs.
    pub fn from_object(
        object: &Object<'static>,
        refs: &Arc<RefList>,
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
        })
    }
}

/// Convert a napi error raised by a hook call into an anyhow error.
fn hook_error(error: napi::Error) -> anyhow::Error {
    anyhow::Error::from(error)
}

/// Serialize `ast` to the ESTree JSON string hooks receive: the spans are
/// converted from byte offsets to UTF-16 code units, matching JS strings.
fn ast_to_estree_json(ast: &Ast) -> String {
    let json: String = oxc_estree_codec::program_to_json(
        ast.program(),
        oxc_estree_codec::ProgramToJsonOptions::new(),
    );

    let spans: SpanMap = SpanMap::new(ast.source());

    spans_to_utf16(&spans, &json)
}

/// Parse a hook's JSON-string source map into an owned map.
fn parse_json_map(json: &str) -> anyhow::Result<SourceMap> {
    Ok(oxc_sourcemap::OwnedSourceMap::from_json_string(json)
        .map_err(anyhow::Error::msg)?
        .into_inner())
}

/// Rebuild an owned [`Ast`] from a `parse` hook's returned ESTree program.
///
/// `program` is the JS-supplied ESTree tree (its spans are UTF-16 code units),
/// `code` its own source text, and `map` an optional incremental source map as
/// a JSON string. The spans are mapped back to byte offsets against `code`
/// before the codec rebuilds the program into a fresh arena.
fn ast_from_parse_output(
    program: &serde_json::Value,
    code: String,
    map: Option<String>,
    source_type: SourceType,
) -> anyhow::Result<Ast> {
    let json: String = serde_json::to_string(program)?;

    let source: Arc<str> = Arc::from(code);

    let spans: SpanMap = SpanMap::new(&source);

    let bytes: String = spans_to_bytes(&spans, &json);

    let input_map: Option<SourceMap> = match map {
        | Some(json) => Some(parse_json_map(&json)?),
        | None => None,
    };

    Ast::try_from_source(
        source,
        source_type,
        input_map,
        |source_text: &str, allocator: &oxc::allocator::Allocator| {
            oxc_estree_codec::json_to_program(
                &bytes,
                oxc_estree_codec::JsonToProgramOptions {
                    allocator,
                    source_type,
                    source_text,
                },
            )
        },
    )
    .map_err(anyhow::Error::from)
}

/// The JS-thread unmarshalling of an `options` hook return value into the
/// pipeline bag.
///
/// Runs ON the JS thread (inside the TSFN callback, which provides the
/// `Env`): the returned plugin descriptors must be bridged where JS objects
/// are reachable. `null`/`undefined` = no change. A returned object is the
/// FULL bag: omitted scalar fields stay `None` ("leave unspecified", resolved
/// later by the pipeline), and every entry of the `plugins` array is bridged
/// (builtins dispatch by name; JS plugin objects are wrapped into fresh
/// `JsPlugin` adapters whose rooted descriptors join the compile's release
/// list).
fn unmarshal_options(
    output: Option<JsOptionsOutput>,
    env: &Env,
    refs: &Arc<RefList>,
) -> anyhow::Result<Option<OptionsArgs>> {
    let Some(output) = output else {
        return Ok(None);
    };

    // The descriptor references arrive ALREADY rooted: the `Option<
    // JsOptionsOutput>` conversion (`ObjectRef`'s `FromNapiValue`) created a
    // `napi_create_reference` root per descriptor, owned by `ctx.value` in
    // the resolution callback. They must move onto the release list AS-IS:
    // re-rooting them with `SharedRef::new` (`create_ref`) would leave the
    // conversion-created root un-unref'd when the callback's value drops
    // (`ObjectRef`'s `Drop` is a no-op), LEAKING one napi_ref per descriptor
    // per options call. `to_plugins` wraps the SAME roots (the entry bridge
    // in `compile` does the same).
    //
    // Parse the scalar fields BEFORE touching the descriptor table: the
    // returned bag is the new CURRENT bag, and its table is only replaced
    // once every fallible step has succeeded, so any error leaves the view
    // untouched (the compile aborts and the release list still drains every
    // rooted descriptor).
    let language: Option<telarel_common::Language> = match &output.language {
        | Some(text) => parse_language(text).map_err(hook_error)?,
        | None => None,
    };

    let source_type: Option<telarel_common::SourceType> =
        match &output.source_type {
            | Some(text) => parse_source_type(text).map_err(hook_error)?,
            | None => None,
        };

    let source_map: Option<telarel_common::SourceMapOptions> =
        match &output.source_map {
            | Some(value) => Some(
                crate::_types::options::parse_source_map_options(value)
                    .map_err(hook_error)?,
            ),
            | None => None,
        };

    // The returned bag REPLACES the whole plugin list (the wrapper echoes
    // the payload's `plugins` array): an omitted field means the default
    // EMPTY list (see `JsOptionsOutput`'s docs). `to_plugins` bridges the
    // whole bag and REPLACES the current descriptor view; on failure it
    // releases the rooted references and leaves the view untouched.
    let plugins: Vec<telarel_plugin::SharedPluginable> =
        to_plugins(env, output.plugins.into_iter().flatten().collect(), refs)
            .map_err(hook_error)?;

    let args: OptionsArgs = OptionsArgs {
        options: ResolvableOptions {
            cwd: output.cwd,
            file: output.file,
            language,
            source_type,
            source_map,
        },
        plugins,
    };

    Ok(Some(args))
}

/// The raw return value of the TSFN bridging an `options` hook.
///
/// The RAW `options` hook is an async function returning
/// `Promise<Option<JsOptionsOutput>>`; the TSFN delivers the RAW promise
/// object to the JS-thread callback, which wires the resolution bridging.
pub type OptionsReturn = Unknown<'static>;

/// Bridge a promise's resolution ON the JS thread, forwarding the outcome.
///
/// Takes over the promise machinery: `PromiseRaw::then` registers a Rust
/// callback invoked on the JS thread with the resolved, CONVERTED
/// `Option<JsOptionsOutput>` (the bag's plugin refs are rooted there);
/// `unmarshal_options` bridges the late plugins and the outcome is sent to
/// the waiting worker. Rejections surface the original napi error.
///
/// The raw hook's return is validated with `napi_is_promise` BEFORE the
/// wiring: [`napi::bindgen_prelude::PromiseRaw::new`] wraps the raw
/// `napi_value` UNCHECKED, and calling `then` on a non-promise (a plain
/// value the (unwrapped) raw hook returned directly, or an object without a
/// callable `then`) would fail — leaving the worker blocked on the outcome
/// channel and the returned error a FATAL exception in napi's TSFN callback
/// (a process abort). Every failure below therefore sends the error on the
/// outcome channel FIRST (so the worker's `recv` always settles) and
/// returns `Ok(())` (so napi does not fatal).
fn await_options_promise(
    value: Unknown,
    env: Env,
    refs: Arc<RefList>,
    sender: Sender<anyhow::Result<Option<OptionsArgs>>>,
) -> napi::Result<()> {
    use napi::bindgen_prelude::{CallbackContext, JsValue, PromiseRaw};

    // The conversion is the JS-value passthrough (infallible in practice);
    // a failure must STILL settle the channel (see this function's docs).
    let raw_value: sys::napi_value = match unsafe {
        <Unknown as ToNapiValue>::to_napi_value(env.raw(), value)
    } {
        | Ok(raw_value) => raw_value,
        | Err(error) => {
            let _ = sender.send(Err(hook_error(error)));

            return Ok(());
        },
    };

    // Validate BEFORE wiring: `PromiseRaw::new` performs NO promise check.
    let is_promise: bool = value.is_promise().unwrap_or(false);

    if !is_promise {
        let _ = sender.send(Err(anyhow::anyhow!(
            "`options` hook must return a Promise (an async function), \
             got a non-promise value"
        )));

        return Ok(());
    }

    let raw: PromiseRaw<'_, Option<JsOptionsOutput>> =
        PromiseRaw::new(env.raw(), raw_value);

    let resolve_sender = sender.clone();

    let reject_sender = sender.clone();

    let resolve = move |ctx: CallbackContext<Option<JsOptionsOutput>>| {
        let outcome = unmarshal_options(ctx.value, &ctx.env, &refs);

        let _ = resolve_sender
            .send(outcome.map_err(|error| anyhow::anyhow!("{error:#}")));

        Ok(())
    };

    let reject = move |ctx: CallbackContext<Unknown>| {
        let error: napi::Error = ctx.value.into();

        let _ = reject_sender.send(Err(hook_error(error)));

        Ok(())
    };

    // `then`/`catch` failures leave the worker's `recv` hanging and, surfaced
    // from this callback, become a fatal exception: error the channel and
    // swallow the napi error instead.
    if let Err(error) = raw.then(resolve).and_then(|wired| wired.catch(reject))
    {
        let _ = sender.send(Err(hook_error(error)));
    }

    Ok(())
}

/// The `options` hook call: unmarshal the resolution ON the JS thread.
///
/// `call_async_catch` awaits the TSFN on the WORKER and returns the RAW
/// converted return, but a returned bag's plugin descriptors must be
/// bridged where JS objects are reachable (the JS thread). This hook uses
/// `call_with_return_value` instead: its callback runs ON the JS thread
/// with an `Env`, wires the PROMISE resolution bridging, and the outcome
/// travels back through a channel.
///
/// The RAW `options` hook is an ASYNC function: it returns a
/// `Promise<Option<JsOptionsOutput>>` (the wrapper awaits the user hook).
/// [`napi::bindgen_prelude::PromiseRaw::then`] registers a RUST callback
/// invoked ON the JS thread with the RESOLVED, CONVERTED value (the bag's
/// plugin descriptor refs are rooted there) and an `Env`.
///
/// A JS throw inside the hook rejects the promise; captured as the compile
/// error via the rejection callback.
async fn call_options_tsfn(
    tsfn: &OptionsTsfn,
    call: OptionsCall,
    refs: &Arc<RefList>,
) -> anyhow::Result<Option<OptionsArgs>> {
    // The options outcome channel: multi-producer (the resolve and reject
    // callbacks each hold a sender), single-consumer (the worker).
    type OptionsOutcome = anyhow::Result<Option<OptionsArgs>>;

    let (sender, receiver): (Sender<OptionsOutcome>, _) =
        std::sync::mpsc::channel();

    // The closure outlives this call (it runs when the promise resolves),
    // so the SHARED list moves in as an owned handle.
    let refs: Arc<RefList> = Arc::clone(refs);

    let status: napi::Status = tsfn.call_with_return_value(
        call,
        ThreadsafeFunctionCallMode::NonBlocking,
        move |output: napi::Result<OptionsReturn>, env: Env| {
            match output {
                | Err(error) => {
                    // The promise was never created (a synchronous throw
                    // inside the raw hook surfaced as a pending exception):
                    // surface it as the hook error.
                    let _ = sender.send(Err(hook_error(error)));

                    Ok(())
                },
                | Ok(value) => await_options_promise(value, env, refs, sender),
            }
        },
    );

    if status == napi::Status::Closing {
        return Err(anyhow::anyhow!(
            "options hook: threadsafe function closed"
        ));
    }

    // The outcome arrives via the JS thread's promise machinery; a plain
    // blocking receive on the worker thread is safe (the SINGLE-task
    // worker's runtime is parked inside `block_on`, and the JS thread is
    // free to drive the promise to resolution).
    std::sync::mpsc::Receiver::recv(&receiver)
        .map_err(|_| anyhow::anyhow!("options hook: callback channel closed"))?
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

            let call: OptionsCall = OptionsCall {
                cwd: SharedStr::new(args.options.cwd.as_deref().unwrap_or("")),
                file: SharedStr::new(
                    args.options.file.as_deref().unwrap_or(""),
                ),
                language: SharedStr::new(language_label(args.options.language)),
                source_type: SharedStr::new(source_type_label(
                    args.options.source_type,
                )),
                plugins: self.refs.bag_snapshot(),
            };

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

            // TEMPORARY ADAPTER (Task-4 deletion target): `ResolvedOptions` no
            // longer carries `code`; read the compile input back from the
            // shared per-compile state so the JS-observable `start` payload is
            // unchanged from before the reshape.
            let code: SharedStr =
                self.refs.source_code().unwrap_or_else(|| SharedStr::new(""));

            let call: StartCall = StartCall {
                cwd: SharedStr::new(&args.options.cwd),
                file: SharedStr::new(&args.options.file),
                code,
                language: SharedStr::new(language_label(Some(
                    args.options.language,
                ))),
                source_type: SharedStr::new(source_type_label(Some(
                    args.options.source_type,
                ))),
                plugins: args.options.plugins.clone(),
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
                cwd: SharedStr::new(ctx.cwd),
                file: SharedStr::new(ctx.module.file),
                code: args.code.to_owned(),
                language: SharedStr::new(language_label(Some(
                    ctx.module.language,
                ))),
                source_type: SharedStr::new(source_type_label(Some(
                    ctx.module.source_type,
                ))),
                options: JsParserOptions::from(args.options),
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
                // A JS `null`/`void` return: defer to the next hook.
                return Ok(None);
            };

            // The returned AST's spans anchor to its own `code`, so resolve the
            // grammar from the hook's parser options (the same resolution the
            // builtin parser applies) and rebuild the tree against it.
            let grammar: SourceType = telarel_common::resolve_grammar(
                &args.options.file,
                Some(args.options.language),
            );

            let source_type: SourceType = telarel_common::with_module_kind(
                grammar,
                args.options.source_type,
            );

            let JsAstOutput { program, code, map } = replaced.ast;

            let ast: Ast =
                ast_from_parse_output(&program, code, map, source_type)?;

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

            let call: ObserveCall = ObserveCall {
                cwd: SharedStr::new(ctx.cwd),
                file: SharedStr::new(ctx.module.file),
                language: SharedStr::new(language_label(Some(
                    ctx.module.language,
                ))),
                source_type: SharedStr::new(source_type_label(Some(
                    ctx.module.source_type,
                ))),
                ast_json: ast_to_estree_json(args.ast),
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
        args: TransformArgs<'a>,
    ) -> HookFuture<'a, TransformReturn> {
        Box::pin(async move {
            let Some(tsfn) = self.tsfn_transform.as_ref() else {
                return Ok(None);
            };

            // Capture the source type BEFORE serializing; the read-back needs
            // it to anchor the JSON tree.
            let source_type: SourceType = args.ast.source_type();

            // The UTF-16 map is anchored to the AST's own source (the parse
            // input), NOT the module's original code, so it matches the byte
            // spans the codec emits below.
            let ast_json: String = oxc_estree_codec::program_to_json(
                args.ast.program(),
                oxc_estree_codec::ProgramToJsonOptions::new(),
            );

            let spans: SpanMap = SpanMap::new(args.ast.source());

            let ast_json_utf16: String = spans_to_utf16(&spans, &ast_json);

            // The ESTree codec skips `Program.comments`; keep the parser-owned
            // comments to reattach them to the rebuilt program (a JS plugin
            // cannot supply its own).
            let comments: Vec<oxc::ast::ast::Comment> =
                args.ast.program().comments.iter().copied().collect();

            let call: TransformCall = TransformCall {
                cwd: SharedStr::new(ctx.cwd),
                file: SharedStr::new(ctx.module.file),
                language: SharedStr::new(language_label(Some(
                    ctx.module.language,
                ))),
                source_type: SharedStr::new(source_type_label(Some(
                    ctx.module.source_type,
                ))),
                ast_json: ast_json_utf16.clone(),
            };

            let output: Either<
                Promise<Option<JsTransformOutput>>,
                Option<JsTransformOutput>,
            > = tsfn.call_async_catch(call).await.map_err(hook_error)?;

            let unmarshalled: Option<JsTransformOutput> = match output {
                | Either::A(promise) => promise.await.map_err(hook_error)?,
                | Either::B(value) => value,
            };

            let Some(replaced) = unmarshalled else {
                // A JS `null`/`void` return: no change.
                return Ok(None);
            };

            // Defense in depth: a pass-through sends the tree back unchanged,
            // so compare in the same UTF-16 form the plugin saw and skip the
            // read-back.
            if replaced.ast_json == ast_json_utf16 {
                return Ok(None);
            }

            // The returned spans are UTF-16: map them back to byte offsets
            // through the same anchored map before the codec reads them.
            let replaced_bytes: String =
                spans_to_bytes(&spans, &replaced.ast_json);

            // Rebuild the returned tree into a fresh owned AST: the read-back
            // allocates into the new AST's arena and anchors spans against its
            // shared source, so the replacement travels as `Some` exactly like
            // a Rust plugin's replacement.
            let mut ast: Ast = Ast::try_from_source(
                args.ast.source().clone(),
                source_type,
                args.ast.map().cloned(),
                |source_text: &str, allocator: &oxc::allocator::Allocator| {
                    oxc_estree_codec::json_to_program(
                        &replaced_bytes,
                        oxc_estree_codec::JsonToProgramOptions {
                            allocator,
                            source_type,
                            source_text,
                        },
                    )
                },
            )
            .map_err(anyhow::Error::from)?;

            // The rebuilt program's comments are empty: reattach the original
            // set so codegen still emits them.
            if !comments.is_empty() {
                ast.with_mut(
                    |allocator: &oxc::allocator::Allocator,
                     program: &mut oxc::ast::ast::Program<'_>| {
                        let mut rebuilt: oxc::allocator::Vec<
                            '_,
                            oxc::ast::ast::Comment,
                        > = oxc::allocator::Vec::new_in(&allocator);

                        rebuilt.extend(comments.iter().copied());

                        program.comments = rebuilt;
                    },
                );
            }

            Ok(Some(TransformOutput { ast }))
        })
    }

    fn call_transform_meta(&self) -> Option<PluginHookMeta> {
        self.hook_meta_transform
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

            let call: ObserveCall = ObserveCall {
                cwd: SharedStr::new(ctx.cwd),
                file: SharedStr::new(ctx.module.file),
                language: SharedStr::new(language_label(Some(
                    ctx.module.language,
                ))),
                source_type: SharedStr::new(source_type_label(Some(
                    ctx.module.source_type,
                ))),
                ast_json: ast_to_estree_json(args.ast),
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
                cwd: SharedStr::new(ctx.cwd),
                file: SharedStr::new(ctx.module.file),
                language: SharedStr::new(language_label(Some(
                    ctx.module.language,
                ))),
                source_type: SharedStr::new(source_type_label(Some(
                    ctx.module.source_type,
                ))),
                ast_json: ast_to_estree_json(args.ast),
                options: JsGeneratorOptions::from(args.options),
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
                // A JS `null`/`void` return: defer to the next hook.
                return Ok(None);
            };

            let map: Option<SourceMap> = match replaced.map {
                | Some(json) => Some(parse_json_map(&json)?),
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
                cwd: SharedStr::new(ctx.cwd),
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

/// Tests for the ENV-FREE parts of the hook marshaling: the options
/// unmarshalling (no full-bag scalar defaulting; omitted-`plugins` = empty
/// list), the ESTree serialization helpers, and the JSON-string map parsing.
/// The descriptor-loop / promise-wiring paths touch napi FFI (reference
/// rooting, `napi_is_promise`) and are only observable through a JS runtime —
/// that surface is exercised by the options-injection tests in
/// `tests/telarel/test/compile.test.ts`.
#[cfg(test)]
mod tests {
    use telarel_common::{Language, ParseAstOptions, SourceType, parse_ast};

    use crate::plugin::hooks::RefList;

    use super::*;

    // The loop over descriptors only runs when the bag CARRIES plugins, so
    // the env is never dereferenced here; `Env::from_raw(null)` keeps the
    // test free of a JS runtime.
    fn env() -> Env {
        napi::Env::from_raw(std::ptr::null_mut())
    }

    fn output(
        cwd: Option<String>,
        file: Option<String>,
        language: Option<String>,
        source_type: Option<String>,
    ) -> JsOptionsOutput {
        JsOptionsOutput {
            cwd,
            file,
            language,
            source_type,
            source_map: None,
            plugins: None,
        }
    }

    fn fixture_ast(code: &str) -> Ast {
        parse_ast(
            code.to_owned(),
            ParseAstOptions {
                cwd: String::from("/repo"),
                file: String::from("a.ts"),
                language: None,
                source_type: None,
                input_map: None,
            },
        )
        .expect("fixture parses")
    }

    fn sample_map() -> SourceMap {
        let mut builder: oxc_sourcemap::SourceMapBuilder<'_> =
            oxc_sourcemap::SourceMapBuilder::default();

        builder.set_file("upstream.ts");

        let source_id: u32 =
            builder.add_source_and_content("upstream.ts", "const a = 1;");

        builder.add_token(0, 0, 0, 0, Some(source_id), None);

        builder.into_owned_sourcemap().into_inner()
    }

    #[test]
    fn test_unmarshal_options_null_is_none() {
        let env: Env = env();
        let refs: Arc<RefList> = Arc::new(RefList::new());

        let outcome = unmarshal_options(None, &env, &refs).unwrap();

        assert!(outcome.is_none());
    }

    #[test]
    fn test_unmarshal_options_bag_keeps_omitted_fields_none() {
        let env: Env = env();
        let refs: Arc<RefList> = Arc::new(RefList::new());

        let outcome = unmarshal_options(
            Some(output(None, None, None, None)),
            &env,
            &refs,
        )
        .unwrap()
        .expect("a returned bag is Some");

        assert_eq!(outcome.options.cwd, None);
        assert_eq!(outcome.options.file, None);
        assert_eq!(outcome.options.language, None);
        assert_eq!(outcome.options.source_type, None);
        assert!(outcome.options.source_map.is_none());
        assert!(outcome.plugins.is_empty());
    }

    #[test]
    fn test_unmarshal_options_bag_parses_source_map() {
        let env: Env = env();
        let refs: Arc<RefList> = Arc::new(RefList::new());

        let mut bag: JsOptionsOutput = output(None, None, None, None);

        bag.source_map = Some(serde_json::json!({
            "input": "inline",
            "output": { "mode": "both" },
        }));

        let outcome = unmarshal_options(Some(bag), &env, &refs)
            .unwrap()
            .expect("a returned bag is Some");

        let source_map =
            outcome.options.source_map.expect("the bag's sourceMap is carried");

        assert!(matches!(
            source_map.input,
            Some(telarel_common::InputSourceMap::Mode(
                telarel_common::InputSourceMapMode::Inline
            ))
        ));

        assert!(matches!(
            source_map.output,
            Some(telarel_common::OutputSourceMap::Options(
                telarel_common::OutputSourceMapOptions {
                    mode: telarel_common::OutputSourceMapMode::Both,
                }
            ))
        ));
    }

    #[test]
    fn test_unmarshal_options_bag_invalid_source_map_errors() {
        let env: Env = env();
        let refs: Arc<RefList> = Arc::new(RefList::new());

        let mut bag: JsOptionsOutput = output(None, None, None, None);

        bag.source_map = Some(serde_json::json!({ "output": "sideways" }));

        let error: anyhow::Error = unmarshal_options(Some(bag), &env, &refs)
            .expect_err("an invalid sourceMap is the hook error");

        assert!(
            error.to_string().contains("invalid sourceMap.output"),
            "{error}"
        );
    }

    #[test]
    fn test_unmarshal_options_bag_fields_and_labels() {
        let env: Env = env();
        let refs: Arc<RefList> = Arc::new(RefList::new());

        let outcome = unmarshal_options(
            Some(output(
                Some("/tmp".to_owned()),
                Some("app.ts".to_owned()),
                Some("ts".to_owned()),
                Some("module".to_owned()),
            )),
            &env,
            &refs,
        )
        .unwrap()
        .expect("a returned bag is Some");

        assert_eq!(outcome.options.cwd.as_deref(), Some("/tmp"));
        assert_eq!(outcome.options.file.as_deref(), Some("app.ts"));
        assert_eq!(outcome.options.language, Some(Language::TS));
        assert_eq!(outcome.options.source_type, Some(SourceType::Module));
        assert!(outcome.plugins.is_empty());
    }

    #[test]
    fn test_unmarshal_options_omitted_plugins_empties_the_bag_table() {
        let env: Env = env();
        let refs: Arc<RefList> = Arc::new(RefList::new());

        refs.set_bag(vec![
            crate::plugin::hooks::SharedRef::stub(),
            crate::plugin::hooks::SharedRef::stub(),
        ]);

        assert_eq!(refs.bag_snapshot().len(), 2);

        unmarshal_options(Some(output(None, None, None, None)), &env, &refs)
            .unwrap()
            .expect("a returned bag is Some");

        assert_eq!(refs.bag_snapshot().len(), 0);
    }

    /// The contract this fix pins: the `options` payload is the CURRENT
    /// bag's descriptor list, NOT the accumulating release list. Two fixpoint
    /// passes echo the SAME two descriptors; the release list grows every
    /// pass, but the bag snapshot stays at two.
    ///
    /// Limitation: the descriptor-loop half of `unmarshal_options` touches
    /// napi FFI (`materialize` → `get_value`) and cannot run without a JS
    /// runtime, so the returned-bag table update is modelled with the same
    /// `set_bag` calls the loop makes. The omitted-`plugins` reset is
    /// exercised through the real `unmarshal_options` above.
    #[test]
    fn test_bag_table_tracks_current_bag_not_release_list() {
        let refs: RefList = RefList::new();

        let first: Vec<crate::plugin::hooks::SharedRef> =
            (0..2).map(|_| crate::plugin::hooks::SharedRef::stub()).collect();

        for descriptor in &first {
            refs.push_ref(descriptor.clone());
        }

        refs.set_bag(first);

        assert_eq!(refs.bag_snapshot().len(), 2);
        assert_eq!(refs.len(), 2);

        let echoed: Vec<crate::plugin::hooks::SharedRef> =
            (0..2).map(|_| crate::plugin::hooks::SharedRef::stub()).collect();

        for descriptor in &echoed {
            refs.push_ref(descriptor.clone());
        }

        refs.set_bag(echoed);

        assert_eq!(refs.bag_snapshot().len(), 2);
        assert_eq!(refs.len(), 4);
    }

    #[test]
    fn test_unmarshal_options_invalid_language_errors() {
        let env: Env = env();
        let refs: Arc<RefList> = Arc::new(RefList::new());

        let outcome = unmarshal_options(
            Some(output(None, None, Some("json".to_owned()), None)),
            &env,
            &refs,
        );

        let error: anyhow::Error =
            outcome.expect_err("an invalid language is the hook error");

        assert!(error.to_string().contains("invalid language"), "{error}");
    }

    #[test]
    fn test_unmarshal_options_invalid_source_type_errors() {
        let env: Env = env();
        let refs: Arc<RefList> = Arc::new(RefList::new());

        let outcome = unmarshal_options(
            Some(output(None, None, None, Some("json".to_owned()))),
            &env,
            &refs,
        );

        let error: anyhow::Error =
            outcome.expect_err("an invalid source type is the hook error");

        assert!(error.to_string().contains("invalid source type"), "{error}");
    }

    #[test]
    fn test_ast_to_estree_json_ascii_is_identity() {
        let ast: Ast = fixture_ast("const a = 1;");

        let json: String = ast_to_estree_json(&ast);

        assert!(json.contains(r#""type":"Program""#), "{json}");
        assert!(json.contains(r#""start":0"#), "{json}");
    }

    #[test]
    fn test_ast_from_parse_output_round_trips_ascii() {
        let ast: Ast = fixture_ast("const a = 1;");

        let json: String = ast_to_estree_json(&ast);

        let program: serde_json::Value =
            serde_json::from_str(&json).expect("serialized program parses");

        let rebuilt: Ast = ast_from_parse_output(
            &program,
            String::from("const a = 1;"),
            None,
            ast.source_type(),
        )
        .expect("read-back succeeds");

        assert_eq!(rebuilt.source().as_ref(), "const a = 1;");
        assert_eq!(rebuilt.program().source_text, "const a = 1;");
        assert!(rebuilt.map().is_none());
    }

    #[test]
    fn test_ast_from_parse_output_converts_utf16_spans_to_bytes() {
        // `日` is three UTF-8 bytes but one UTF-16 code unit: a read-back that
        // forgot the conversion would misplace every span after it.
        let ast: Ast = fixture_ast("日a");

        let json: String = ast_to_estree_json(&ast);

        let program: serde_json::Value =
            serde_json::from_str(&json).expect("serialized program parses");

        let rebuilt: Ast = ast_from_parse_output(
            &program,
            String::from("日a"),
            None,
            ast.source_type(),
        )
        .expect("read-back succeeds");

        assert_eq!(rebuilt.source().as_ref(), "日a");

        // A correct read-back re-serializes to the exact UTF-16 JSON it came
        // from; a byte/UTF-16 mix-up would shift every span after `日`.
        assert_eq!(ast_to_estree_json(&rebuilt), json);
    }

    #[test]
    fn test_ast_from_parse_output_attaches_returned_map() {
        let ast: Ast = fixture_ast("const a = 1;");

        let json: String = ast_to_estree_json(&ast);

        let program: serde_json::Value =
            serde_json::from_str(&json).expect("serialized program parses");

        let input_map: SourceMap = sample_map();

        let rebuilt: Ast = ast_from_parse_output(
            &program,
            String::from("const a = 1;"),
            Some(input_map.to_json_string()),
            ast.source_type(),
        )
        .expect("read-back succeeds");

        let carried: &SourceMap =
            rebuilt.map().expect("the returned map is attached");

        assert_eq!(carried.to_json_string(), input_map.to_json_string());
    }

    #[test]
    fn test_ast_from_parse_output_invalid_map_errors() {
        let ast: Ast = fixture_ast("const a = 1;");

        let json: String = ast_to_estree_json(&ast);

        let program: serde_json::Value =
            serde_json::from_str(&json).expect("serialized program parses");

        let outcome: anyhow::Result<Ast> = ast_from_parse_output(
            &program,
            String::from("const a = 1;"),
            Some(String::from("not json")),
            ast.source_type(),
        );

        assert!(outcome.is_err());
    }

    #[test]
    fn test_parse_json_map_round_trips() {
        let map: SourceMap = sample_map();

        let parsed: SourceMap =
            parse_json_map(&map.to_json_string()).expect("map parses");

        assert_eq!(parsed.to_json_string(), map.to_json_string());
    }

    #[test]
    fn test_parse_json_map_invalid_errors() {
        assert!(parse_json_map("not json").is_err());
    }
}
