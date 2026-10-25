use std::borrow::Cow;
use std::path::PathBuf;
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
    Ast, AstResult, CodeResult, HookUsage, Language, ResolvableOptions,
    SourceMap,
};
use telarel_plugin::__internal::HookFuture;
use telarel_plugin::{
    CommonPluginContext, EndArgs, FinalizeArgs, GenerateArgs, GenerateReturn,
    NotifyReturn, OptionsArgs, ParseArgs, ParseReturn, PluginContext,
    PluginError, PluginHookMeta, Pluginable, PrepareArgs, StartArgs,
    TransformArgs, TransformOutput, TransformReturn,
};

use crate::_types::ast::JsAst;
use crate::_types::path::path_label;
use crate::_types::plugin::hooks::{
    JsCodeOutput, JsGeneratorHookOptions, JsOptionsOutput, JsParseOutput,
    JsParserHookOptions, JsResolvedOptions, JsTransformOutput,
    optional_output_options_to_js, optional_source_map_options_to_js,
};
use crate::_types::plugin::order::read_hook_meta;
use crate::_types::sourcemap::JsSourceMap;
use crate::ast::{
    ast_from_json, ast_to_estree_json, js_source_map_to_owned, js_to_ast,
    source_map_to_js, source_type_for_generate, source_type_for_options,
};
use crate::comment;
use crate::errors::plugin::decode_plugin_error;
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

/// Convert a napi error raised by a hook call into a plugin error.
///
/// A JS `PluginError` travels as a private envelope in the message; ordinary
/// throwables keep their existing `napi::Error` rendering.
fn hook_error(error: napi::Error) -> PluginError {
    match decode_plugin_error(&error.reason) {
        | Some(decoded) => decoded,
        | None => PluginError::new(error.to_string()),
    }
}

/// Convert a read-back failure into a plugin error without an `anyhow`
/// conversion impl in the plugin crate.
fn anyhow_to_plugin_error(error: anyhow::Error) -> PluginError {
    PluginError::new(format!("{error:#}"))
}

/// Tree-hook payload: JSON, source, input map, grammar label.
fn ast_hook_parts(ast: &Ast) -> (String, String, Option<JsSourceMap>, String) {
    (
        ast_to_estree_json(ast),
        ast.source().to_string(),
        ast.map().map(source_map_to_js),
        ast.language().as_str().to_string(),
    )
}

/// The transform read-back grammar: a returned label wins; absent means the
/// input AST's grammar still anchors the rebuild.
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

/// Unmarshal an `options` hook return into the pipeline bag ON the JS thread,
/// where the returned plugin descriptors are reachable. `null`/`undefined` is
/// no change; a returned object is the FULL bag: omitted scalars stay
/// unspecified and every `plugins` entry is bridged (rooted descriptors join
/// the compile's release list).
fn unmarshal_options(
    output: Option<JsOptionsOutput>,
    env: &Env,
    refs: &Arc<RefList>,
) -> std::result::Result<Option<OptionsArgs>, PluginError> {
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
            cwd: output.cwd.map(PathBuf::from),
            file: output.file,
            language,
            source_type,
            output: output_options,
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

/// Bridge a promise's resolution on the JS thread. Every failure settles the
/// outcome channel first; an escaping napi error here aborts the process.
fn await_options_promise(
    value: Unknown,
    env: Env,
    refs: Arc<RefList>,
    sender: Sender<std::result::Result<Option<OptionsArgs>, PluginError>>,
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

    // `then`/`catch` failures leave the worker's `recv` hanging and, surfaced
    // from this callback, become a fatal exception: error the channel and
    // swallow the napi error instead.
    if let Err(error) = raw.then(resolve).and_then(|wired| wired.catch(reject))
    {
        let _ = sender.send(Err(hook_error(error)));
    }

    Ok(())
}

/// The `options` hook call. It resolves on the JS thread because a returned
/// bag's plugin descriptors must be bridged where JS objects are reachable;
/// the outcome travels back to the worker through a channel.
async fn call_options_tsfn(
    tsfn: &OptionsTsfn,
    call: OptionsCall,
    refs: &Arc<RefList>,
) -> std::result::Result<Option<OptionsArgs>, PluginError> {
    // The options outcome channel: multi-producer (the resolve and reject
    // callbacks each hold a sender), single-consumer (the worker).
    type OptionsOutcome = std::result::Result<Option<OptionsArgs>, PluginError>;

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
        return Err(PluginError::new(
            "options hook: threadsafe function closed",
        ));
    }

    // The outcome arrives via the JS thread's promise machinery; a plain
    // blocking receive on the worker thread is safe (the SINGLE-task
    // worker's runtime is parked inside `block_on`, and the JS thread is
    // free to drive the promise to resolution).
    std::sync::mpsc::Receiver::recv(&receiver).map_err(|_| {
        PluginError::new("options hook: callback channel closed")
    })?
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
                cwd: args
                    .options
                    .cwd
                    .as_deref()
                    .map(|cwd| SharedStr::new(&path_label(cwd))),
                file: args.options.file.as_deref().map(SharedStr::new),
                language: SharedStr::new(language_label(args.options.language)),
                source_type: SharedStr::new(source_type_label(
                    args.options.source_type,
                )),
                output: optional_output_options_to_js(
                    args.options.output.as_ref(),
                ),
                source_map: optional_source_map_options_to_js(
                    args.options.source_map.as_ref(),
                ),
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

            // Parser options own the grammar: a returned AST's language label
            // is deliberately ignored here, because its spans must be read
            // under the grammar the builtin parser resolved.
            let source_type: SourceType = source_type_for_options(
                &args.options.file,
                Some(args.options.language),
                args.options.source_type,
            );

            let ast: Ast = js_to_ast(replaced.ast.into(), source_type)
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

            // The UTF-16 map is anchored to the AST's own source (the parse
            // input), NOT the module's original code, so it matches the byte
            // spans the codec emits.
            let (ast_json_utf16, source_code, source_map, ast_language): (
                String,
                String,
                Option<JsSourceMap>,
                String,
            ) = ast_hook_parts(&args.ast);

            let call: TransformCall = TransformCall {
                cwd: SharedStr::new(&path_label(ctx.cwd)),
                file: SharedStr::new(ctx.module.file),
                language: SharedStr::new(language_label(Some(
                    ctx.module.language,
                ))),
                source_type: SharedStr::new(source_type_label(Some(
                    ctx.module.source_type,
                ))),
                ast_json: ast_json_utf16.clone(),
                code: source_code.clone(),
                map: source_map.clone(),
                ast_language: ast_language.clone(),
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
                return Ok(None);
            };

            // A returned grammar wins; absent means the input AST's grammar
            // still anchors the read-back.
            let language: Language = transform_read_back_language(
                replaced.language.as_deref(),
                args.ast.language(),
            )?;

            // Defense in depth: a pass-through sends the tree back unchanged,
            // so compare every AST field in the form the plugin saw and skip
            // the read-back. The language comparison keeps a grammar-only
            // change from being short-circuited.
            if replaced.ast_json == ast_json_utf16
                && replaced.code == source_code
                && replaced.map == source_map
                && language == args.ast.language()
            {
                // A pass-through keeps parser anchors, which are token
                // boundaries rather than codegen print sites. Sanitize the
                // owned AST in place to keep oxc-only annotation flags.
                let mut ast: Ast = args.ast;

                if comment::reanchor_ast(&mut ast) {
                    return Ok(Some(TransformOutput { ast }));
                }

                return Ok(None);
            }

            // The returned program's `sourceType` owns the module kind; the
            // chosen grammar only supplies the unambiguous parser input.
            let source_type: SourceType = source_type_for_generate(language);

            let input_map: Option<SourceMap> = match replaced.map {
                | Some(map) => Some(
                    js_source_map_to_owned(&map)
                        .map_err(anyhow_to_plugin_error)?,
                ),
                | None => None,
            };

            // Rebuild the returned tree into a fresh owned AST: the read-back
            // allocates into the new AST's arena and anchors spans against the
            // returned source and map, so the replacement travels as `Some`
            // exactly like a Rust plugin's replacement.
            let ast: Ast = ast_from_json(
                &replaced.ast_json,
                Arc::from(replaced.code),
                input_map,
                source_type,
            )
            .map_err(anyhow_to_plugin_error)?;

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

/// Tests for the ENV-FREE parts of the hook marshaling: the options
/// unmarshalling (no full-bag scalar defaulting; omitted-`plugins` = empty
/// list) and the bag/release-list bookkeeping. The ESTree serialization and
/// map parsing live in `crate::ast` and are tested there. The descriptor-loop /
/// promise-wiring paths touch napi FFI (reference rooting, `napi_is_promise`)
/// and are only observable through a JS runtime — that surface is exercised by
/// the options-injection tests in `tests/telarel/test/compile.test.ts`.
#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use telarel_common::{
        Language, OutputOptions, SourceMapOptions, SourceType,
    };

    use crate::_types::ast::JsAstInput;
    use crate::ast::ast_from_program;
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
            output: None,
            source_map: None,
            plugins: None,
        }
    }

    #[test]
    fn test_hook_error_decodes_a_plugin_envelope() {
        let reason: String =
            String::from("__TELAREL_PLUGIN_ERROR__:{\"message\":\"boom\"}");

        let error: PluginError =
            hook_error(napi::Error::new(napi::Status::GenericFailure, reason));

        assert_eq!(error.message(), "boom");
        assert!(error.source().is_none());
    }

    #[test]
    fn test_hook_error_keeps_ordinary_napi_rendering() {
        let error: PluginError = hook_error(napi::Error::new(
            napi::Status::GenericFailure,
            "kaboom",
        ));

        assert_eq!(error.message(), "GenericFailure, kaboom");
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
        assert!(outcome.options.output.is_none());
        assert!(outcome.options.source_map.is_none());
        assert!(outcome.plugins.is_empty());
    }

    #[test]
    fn test_unmarshal_options_bag_parses_output() {
        let env: Env = env();
        let refs: Arc<RefList> = Arc::new(RefList::new());

        let mut bag: JsOptionsOutput = output(None, None, None, None);

        bag.output = Some(crate::_types::options::JsOutputOptions {
            file: Some(String::from("dist/index.js")),
        });

        let outcome: OptionsArgs = unmarshal_options(Some(bag), &env, &refs)
            .unwrap()
            .expect("a returned bag is Some");

        let output: OutputOptions =
            outcome.options.output.expect("the bag's output is carried");

        assert_eq!(output.file.as_deref(), Some("dist/index.js"));
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
                    url: None,
                }
            ))
        ));
    }

    #[test]
    fn test_unmarshal_options_bag_parses_source_map_paths() {
        let env: Env = env();
        let refs: Arc<RefList> = Arc::new(RefList::new());

        let mut bag: JsOptionsOutput = output(None, None, None, None);

        bag.source_map = Some(serde_json::json!({
            "input": {
                "mode": "external",
                "path": "maps/in.map",
            },
            "output": {
                "mode": "external",
                "url": "dist/out.js.map",
            },
        }));

        let outcome: OptionsArgs = unmarshal_options(Some(bag), &env, &refs)
            .unwrap()
            .expect("a returned bag is Some");

        let source_map: SourceMapOptions =
            outcome.options.source_map.expect("the bag's sourceMap is carried");

        assert!(matches!(
            source_map.input,
            Some(telarel_common::InputSourceMap::Options(
                telarel_common::InputSourceMapOptions {
                    mode: telarel_common::InputSourceMapMode::External,
                    path: Some(path),
                }
            )) if path == std::path::Path::new("maps/in.map")
        ));

        assert!(matches!(
            source_map.output,
            Some(telarel_common::OutputSourceMap::Options(
                telarel_common::OutputSourceMapOptions {
                    mode: telarel_common::OutputSourceMapMode::External,
                    url: Some(url),
                }
            )) if url == "dist/out.js.map"
        ));
    }

    #[test]
    fn test_unmarshal_options_bag_invalid_source_map_errors() {
        let env: Env = env();
        let refs: Arc<RefList> = Arc::new(RefList::new());

        let mut bag: JsOptionsOutput = output(None, None, None, None);

        bag.source_map = Some(serde_json::json!({ "output": "sideways" }));

        let error: PluginError = unmarshal_options(Some(bag), &env, &refs)
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

        assert_eq!(outcome.options.cwd, Some(PathBuf::from("/tmp")));
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

    /// The contract: the `options` payload is the CURRENT bag's descriptor
    /// list, NOT the accumulating release list. Two fixpoint passes echo the
    /// SAME two descriptors; the release list grows every pass, but the bag
    /// snapshot stays at two.
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

        let error: PluginError =
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

        let error: PluginError =
            outcome.expect_err("an invalid source type is the hook error");

        assert!(error.to_string().contains("invalid source type"), "{error}");
    }

    #[test]
    fn test_transform_read_back_returned_language_wins() {
        let language: Language =
            transform_read_back_language(Some("jsx"), Language::TS)
                .expect("a returned language parses");

        assert_eq!(language, Language::JSX);
    }

    #[test]
    fn test_transform_read_back_absent_language_falls_back_to_input() {
        let language: Language =
            transform_read_back_language(None, Language::TSX)
                .expect("absent language is unset");

        assert_eq!(language, Language::TSX);
    }

    #[test]
    fn test_transform_read_back_empty_language_is_unset() {
        let language: Language =
            transform_read_back_language(Some(""), Language::JS)
                .expect("an empty label is unset");

        assert_eq!(language, Language::JS);
    }

    #[test]
    fn test_transform_read_back_invalid_language_errors() {
        let error: PluginError =
            transform_read_back_language(Some("json"), Language::JS)
                .expect_err("an invalid label is the hook error");

        assert!(error.to_string().contains("invalid language"), "{error}");
    }

    #[test]
    fn test_transform_read_back_defers_module_kind_to_program_source_type() {
        let language: Language =
            transform_read_back_language(Some("tsx"), Language::JS)
                .expect("the returned language parses");

        let source_type: oxc::span::SourceType =
            source_type_for_generate(language);

        assert!(source_type.is_unambiguous(), "{source_type:?}");

        let program: serde_json::Value = serde_json::json!({
            "type": "Program",
            "sourceType": "module",
            "body": [],
            "start": 0,
            "end": 0,
        });

        let ast: Ast =
            ast_from_program(&program, String::new(), None, source_type)
                .expect("the program decodes");

        assert_eq!(ast.language(), Language::TSX);
        assert_eq!(ast.source_type(), SourceType::Module);
        assert!(ast.program().source_type.is_module());
    }

    fn parse_output(
        program: serde_json::Value,
        language: Option<String>,
    ) -> JsParseOutput {
        JsParseOutput {
            ast: JsAstInput {
                program,
                code: String::new(),
                map: None,
                language,
            },
        }
    }

    fn empty_module_program() -> serde_json::Value {
        serde_json::json!({
            "type": "Program",
            "sourceType": "module",
            "body": [],
            "start": 0,
            "end": 0,
        })
    }

    #[test]
    fn test_parse_output_ast_without_language_rebuilds_with_parser_options() {
        let output: JsParseOutput = parse_output(empty_module_program(), None);

        let source_type: oxc::span::SourceType = source_type_for_options(
            "index.ts",
            Some(Language::TS),
            SourceType::Unambiguous,
        );

        let ast: Ast = js_to_ast(output.ast.into(), source_type)
            .expect("an absent returned language is unset");

        assert_eq!(ast.language(), Language::TS);
        assert_eq!(ast.source_type(), SourceType::Module);
    }

    #[test]
    fn test_parse_output_ast_language_is_ignored_for_grammar() {
        let output: JsParseOutput =
            parse_output(empty_module_program(), Some(String::from("jsx")));

        let source_type: oxc::span::SourceType = source_type_for_options(
            "index.ts",
            Some(Language::TS),
            SourceType::Unambiguous,
        );

        let ast: Ast = js_to_ast(output.ast.into(), source_type)
            .expect("a returned language does not block read-back");

        assert_eq!(ast.language(), Language::TS);
        assert!(ast.program().source_type.is_typescript());
    }
}
