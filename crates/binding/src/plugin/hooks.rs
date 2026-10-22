use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use napi::bindgen_prelude::{
    Array, Either, FromNapiValue, Function, JsValue, JsValuesTupleIntoVec,
    Object, ObjectRef, Promise, ToNapiValue, TypeName, Undefined,
    ValidateNapiValue,
};
use napi::sys;
use napi::threadsafe_function::{ThreadsafeCallContext, ThreadsafeFunction};
use napi::{Env, Error, Result, Status};

use crate::_types::ast::JsAst;
use crate::_types::plugin::context::JsPluginContext;
use crate::_types::plugin::hooks::{
    JsAstArgs, JsCodeOutput, JsEndArgs, JsGenerateArgs, JsParseArgs,
    JsParseOutput, JsResolvedOptions, JsStartArgs, JsTransformOutput,
};
use crate::_types::sourcemap::JsSourceMap;

/// A TSFN bridging one JS hook.
///
/// - `Data`: worker-side call data moved to the JS thread.
/// - `Payload`: JS value(s) the hook receives.
/// - `Return`: JS value the hook returns directly (sync) or resolves (async);
///   wrapped in an `Either` over `Promise<Return>` by the hook aliases.
///
/// `CalleeHandled = false`: hook payloads are passed to the JS function as-is
/// (the `true` variant would prepend a Node-style error argument and shift
/// every hook parameter by one). A JS throw inside the hook is still captured:
/// with the `WithCallback` call variant used by `call_async_catch`, the
/// exception is delivered to the awaiting worker as `Err(napi::Error)` (status
/// `PendingException`) so the compile promise rejects with the original
/// message instead of the error escaping as a fatal exception.
pub type Tsfn<Data, Payload, Return> =
    ThreadsafeFunction<Data, Return, Payload, napi::Status, false, false, 0>;

/// A cheap-to-clone shared string: one allocation, shared by a hook call's
/// context and payload through `Arc` refcounts.
///
/// Serializes to JS as a plain UTF-8 string, so hook payload shapes are unchanged.
#[derive(Debug, Clone)]
pub struct SharedStr(Arc<str>);

impl SharedStr {
    /// Create a shared string from a borrowed value.
    pub fn new(value: &str) -> Self {
        Self(Arc::from(value))
    }

    /// Borrow the shared string's contents.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for SharedStr {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl ToNapiValue for SharedStr {
    unsafe fn to_napi_value(
        env: sys::napi_env,
        val: Self,
    ) -> Result<sys::napi_value> {
        unsafe { <&str as ToNapiValue>::to_napi_value(env, val.0.as_ref()) }
    }
}

macro_rules! for_each_hook_slot {
    ($macro_name:ident $(, $extra:tt)*) => {
        $macro_name! {
            $($extra,)*
            {
                (options, Options, crate::plugin::hooks::OptionsCall, Option<crate::_types::plugin::hooks::JsOptionsOutput>, 0),
                (start, Start, crate::plugin::hooks::FnCtx<crate::_types::plugin::hooks::JsStartArgs>, napi::bindgen_prelude::Undefined, 1),
                (parse, Parse, crate::plugin::hooks::FnCtx<crate::_types::plugin::hooks::JsParseArgs>, Option<crate::_types::plugin::hooks::JsParseOutput>, 2),
                (prepare, Prepare, crate::plugin::hooks::FnCtx<crate::_types::plugin::hooks::JsAstArgs>, napi::bindgen_prelude::Undefined, 3),
                (transform, Transform, crate::plugin::hooks::FnCtx<crate::_types::plugin::hooks::JsAstArgs>, Option<crate::_types::plugin::hooks::JsTransformOutput>, 4),
                (finalize, Finalize, crate::plugin::hooks::FnCtx<crate::_types::plugin::hooks::JsAstArgs>, napi::bindgen_prelude::Undefined, 5),
                (generate, Generate, crate::plugin::hooks::FnCtx<crate::_types::plugin::hooks::JsGenerateArgs>, Option<crate::_types::plugin::hooks::JsCodeOutput>, 6),
                (end, End, crate::plugin::hooks::FnCtx<crate::_types::plugin::hooks::JsEndArgs>, napi::bindgen_prelude::Undefined, 7),
            }
        }
    };
}

macro_rules! hook_slots {
    ($rows:tt) => {
        hook_slots_rows!($rows);
    };
}

macro_rules! hook_slots_rows {
    ({ $(($name:ident, $Name:ident, $args:ty, $ret:ty, $slot:literal)),* $(,)? }) => {
        $( // Declarative slot metadata; the `options` hook's `Args` is the
           // `OptionsCall` payload producer.
            #[allow(dead_code)]
            pub mod $name {
            pub const SLOT: usize = $slot;

            pub type Args = $args;

            pub type Return = $ret;
        } )*
    };
}

for_each_hook_slot!(hook_slots);

/// Materialize a rooted object reference into a raw object handle.
pub fn materialize(
    reference: &ObjectRef<false>,
    env: &Env,
) -> Result<Object<'static>> {
    let value: Object<'_> = reference.get_value(env)?;

    Ok(Object::from_raw(env.raw(), value.raw()))
}

/// A shared, rooted object reference.
///
/// [`ObjectRef`] is `Send` but not `Sync`; the reference is only dereferenced
/// (via `get_value`) on the JS thread, so sharing the immutable handle across
/// threads is sound.
///
/// The inner reference is released (unrooted) by [`SharedRef::release`], which
/// the `compile` task calls once the compile finishes; it is idempotent and
/// safe to call while clones are still alive.
#[derive(Clone)]
pub struct SharedRef(Arc<Mutex<Option<ObjectRef<false>>>>);

unsafe impl Send for SharedRef {}

unsafe impl Sync for SharedRef {}

impl SharedRef {
    /// Root a JS object and wrap its reference.
    #[allow(clippy::arc_with_non_send_sync)] // see the type's docs
    pub fn new(object: &Object<'static>) -> Result<Self> {
        Ok(Self(Arc::new(Mutex::new(Some(object.create_ref()?)))))
    }

    /// Wrap an already-rooted object reference.
    #[allow(clippy::arc_with_non_send_sync)] // see the type's docs
    pub fn from_ref(reference: ObjectRef<false>) -> Self {
        Self(Arc::new(Mutex::new(Some(reference))))
    }

    /// Test-only: an inert placeholder reference with no rooted object, for
    /// exercising the bag/release-list bookkeeping without a JS runtime.
    #[cfg(test)]
    pub(crate) fn stub() -> Self {
        Self(Arc::new(Mutex::new(None)))
    }

    /// Materialize the rooted object for use on the JS thread.
    pub fn get(
        &self,
        env: &Env,
    ) -> Result<Object<'static>> {
        let guard: MutexGuard<'_, Option<ObjectRef<false>>> =
            self.0.lock().unwrap_or_else(PoisonError::into_inner);

        match guard.as_ref() {
            | Some(reference) => materialize(reference, env),
            | None => Err(Error::new(
                Status::GenericFailure,
                "rooted object reference was already released".to_owned(),
            )),
        }
    }

    /// Release the rooted reference. Idempotent; clones left behind become
    /// inert (`get` errors instead of touching the released reference).
    pub fn release(
        &self,
        env: &Env,
    ) -> Result<()> {
        let mut guard: MutexGuard<'_, Option<ObjectRef<false>>> =
            self.0.lock().unwrap_or_else(PoisonError::into_inner);

        if let Some(reference) = guard.take() {
            reference.unref(env)?;
        }

        Ok(())
    }
}

/// Release every reference; every reference is released even if one release
/// fails, so no reference leaks on the first-error path.
pub fn release_refs(
    env: &Env,
    refs: &[SharedRef],
) -> Result<()> {
    let mut first_error: Option<napi::Error> = None;

    for reference in refs {
        if let Err(error) = reference.release(env)
            && first_error.is_none()
        {
            first_error = Some(error);
        }
    }

    match first_error {
        | Some(error) => Err(error),
        | None => Ok(()),
    }
}

/// A dynamic, per-compile registry of rooted JS object references.
///
/// Two roles share this handle (it is `Arc`-shared into every site that
/// needs either):
///
/// - `refs` — the RELEASE list. Starts with the entry-time references (plugin
///   array, each plugin object). The `options` hook APPENDS late plugins'
///   references — plugins injected by the fixpoint — as they are wrapped and
///   registered, BEFORE the driver is built;
///   [`crate::tasks::function::FunctionTask::finally`] drains the list once
///   the call settles, so mid-call additions are still released.
/// - `bag_descriptors` — a VIEW of the CURRENT bag's plugin descriptors, one
///   per plugin in the `OptionsArgs` being folded. Entries are
///   `SharedRef` clones that SHARE the same roots as the release list (the
///   table never owns or releases a reference); the view is replaced whole by
///   [`RefList::set_bag`] (entry seed, then each returned bag).
#[derive(Default)]
pub struct RefList {
    /// Rooted references drained in `FunctionTask::finally`.
    refs: Mutex<Vec<SharedRef>>,
    /// The current bag's descriptor view; see the type's docs.
    bag_descriptors: Mutex<Vec<SharedRef>>,
}

unsafe impl Send for RefList {}

unsafe impl Sync for RefList {}

impl RefList {
    /// Create an empty reference list.
    pub fn new() -> Self {
        Self::default()
    }

    /// Snapshot the release-list references, in insertion order.
    pub fn refs(&self) -> Vec<SharedRef> {
        self.refs.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }

    /// Whether the release list has no references.
    pub fn is_empty(&self) -> bool {
        self.refs.lock().unwrap_or_else(PoisonError::into_inner).is_empty()
    }

    /// Number of references on the release list.
    pub fn len(&self) -> usize {
        self.refs.lock().unwrap_or_else(PoisonError::into_inner).len()
    }

    /// Clone the current bag's descriptor view (one entry per plugin, in
    /// bag order).
    pub fn bag_snapshot(&self) -> Vec<SharedRef> {
        self.bag_descriptors
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Root a JS object and push its reference onto the release list.
    pub fn push_object(
        &self,
        object: &Object<'static>,
    ) -> Result<()> {
        self.refs
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(SharedRef::new(object)?);

        Ok(())
    }

    /// Push an already-rooted reference onto the release list.
    pub fn push_ref(
        &self,
        reference: SharedRef,
    ) {
        self.refs
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(reference);
    }

    /// Replace the current bag's descriptor view.
    ///
    /// The descriptors are `SharedRef` clones SHARING the release list's
    /// roots; this table never releases them (the release list owns that
    /// lifecycle). A returned bag REPLACES the view whole — an empty list is
    /// the omitted-`plugins` default. On a partial unmarshal failure the
    /// table is left untouched (the compile aborts and the release list
    /// still drains every rooted descriptor).
    ///
    /// Rust-native plugins returning bags do not flow through this NAPI
    /// surface, so only the entry bag and `options`-returned bags matter here.
    pub fn set_bag(
        &self,
        descriptors: Vec<SharedRef>,
    ) {
        let mut bag: MutexGuard<'_, Vec<SharedRef>> =
            self.bag_descriptors.lock().unwrap_or_else(PoisonError::into_inner);

        *bag = descriptors;
    }

    /// Release every reference on the release list; first release error wins.
    pub fn release(
        &self,
        env: &Env,
    ) -> Result<()> {
        let refs: Vec<SharedRef> = self.refs();

        release_refs(env, &refs)
    }
}

/// Worker-side call data for the `options` hook.
pub struct OptionsCall {
    /// Working directory used to resolve relative paths; absent when unset.
    pub cwd: Option<SharedStr>,
    /// The file being parsed/generated; absent when unset.
    pub file: Option<SharedStr>,
    /// Source language of the file.
    pub language: SharedStr,
    /// Module system used for code generation.
    pub source_type: SharedStr,
    /// The output code settings requested so far, as the JS object;
    /// `None` (unspecified) is omitted from the payload.
    pub output: Option<serde_json::Value>,
    /// The source-map inputs and outputs requested so far, as the JS union;
    /// `None` (unspecified) is omitted from the payload.
    pub source_map: Option<serde_json::Value>,
    /// The CURRENT bag's plugin descriptors, snapshotted at call time; the
    /// JS-thread payload materializes the ARRAY from this exact list (NOT
    /// the accumulating release list — see the binding contract).
    pub plugins: Vec<SharedRef>,
}

impl ToNapiValue for OptionsCall {
    unsafe fn to_napi_value(
        env: sys::napi_env,
        val: Self,
    ) -> Result<sys::napi_value> {
        let env: Env = Env::from_raw(env);

        let mut object: Object<'static> = Object::new(&env)?;

        // Unset values are absent, not empty strings: the pre-resolution bag
        // must preserve "unset" so a hook echoing it keeps the defaults.
        if let Some(cwd) = val.cwd {
            object.set("cwd", cwd)?;
        }

        if let Some(file) = val.file {
            object.set("file", file)?;
        }

        object.set("language", val.language)?;
        object.set("sourceType", val.source_type)?;

        // An omitted `output` means "unspecified" on the wire; only a
        // requested output is transmitted.
        if let Some(output) = val.output {
            object.set("output", output)?;
        }

        // An omitted `sourceMap` means "unspecified" on the wire; only a
        // requested map is transmitted.
        if let Some(source_map) = val.source_map {
            object.set("sourceMap", source_map)?;
        }

        let descriptors: Vec<Object<'static>> = val
            .plugins
            .iter()
            .map(|reference| reference.get(&env))
            .collect::<Result<Vec<Object<'static>>>>()?;

        let plugins: Array<'_> = Array::from_vec(
            &env,
            descriptors.iter().collect::<Vec<&Object<'static>>>(),
        )?;

        object.set("plugins", plugins)?;

        Ok(JsValue::raw(&object))
    }
}

/// TSFN bridging the `options` hook. Its payload is emitted by
/// [`OptionsCall`] (the slot table's `Args`); the
/// `plugins` property is the CURRENT bag's plugin descriptors.
/// The RAW `options` hook is async: it returns a promise carrying
/// `Option<JsOptionsOutput>`, delivered RAW (see
/// [`crate::plugin::pluginable::OptionsReturn`]).
pub type OptionsTsfn =
    Tsfn<OptionsCall, OptionsCall, crate::plugin::pluginable::OptionsReturn>;

/// A worker-side ctx-bearing hook call.
pub trait HookCall: 'static {
    /// JS args payload accompanying the context.
    type Payload: ToNapiValue;

    /// JS return type of the hook (before the sync/async `Either`).
    type Return: FromNapiValue + TypeName + ValidateNapiValue + 'static;

    /// Consume the call data into the JS-facing plugin context and args payload.
    fn into_payload(self) -> Result<(JsPluginContext, Self::Payload)>;
}

/// Build the JS-facing plugin context for a hook call.
pub fn plugin_context(
    cwd: SharedStr,
    file: SharedStr,
    language: SharedStr,
    source_type: SharedStr,
) -> Result<JsPluginContext> {
    Ok(JsPluginContext { cwd, file, language, source_type })
}

/// A hook payload: the plugin context followed by the hook args.
pub struct FnCtx<A>(
    /// Plugin context passed as the first hook argument.
    pub JsPluginContext,
    /// Hook args passed as the second hook argument.
    pub A,
);

impl<A: ToNapiValue> JsValuesTupleIntoVec for FnCtx<A> {
    // `env` is a raw napi handle owned by the JS thread; the trait method is
    // not `unsafe`, matching napi's own `JsValuesTupleIntoVec` impls.
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    fn into_vec(
        self,
        env: sys::napi_env,
    ) -> Result<Vec<sys::napi_value>> {
        Ok(vec![unsafe { ToNapiValue::to_napi_value(env, self.0)? }, unsafe {
            ToNapiValue::to_napi_value(env, self.1)?
        }])
    }
}

/// Worker-side call data for the `start` hook: resolved, read-only options
/// with the settled plugin-name list.
pub struct StartCall {
    /// Working directory used to resolve relative paths.
    pub cwd: SharedStr,
    /// The file being parsed/generated.
    pub file: SharedStr,
    /// Source language of the file.
    pub language: SharedStr,
    /// Module system used for code generation.
    pub source_type: SharedStr,
    /// Options resolved for the hook/stage.
    pub options: JsResolvedOptions,
}

impl HookCall for StartCall {
    type Payload = JsStartArgs;
    type Return = Undefined;

    fn into_payload(self) -> Result<(JsPluginContext, Self::Payload)> {
        let context: JsPluginContext = plugin_context(
            self.cwd.clone(),
            self.file.clone(),
            self.language.clone(),
            self.source_type.clone(),
        )?;

        let payload: JsStartArgs = JsStartArgs { options: self.options };

        Ok((context, payload))
    }
}

/// TSFN bridging the `start` hook.
pub type StartTsfn =
    Tsfn<StartCall, start::Args, Either<Promise<start::Return>, start::Return>>;

/// Worker-side call data for the `parse` hook: the code to parse, the resolved
/// parser options, and their JS-facing form.
pub struct ParseCall {
    /// Working directory used to resolve relative paths.
    pub cwd: SharedStr,
    /// The file being parsed/generated.
    pub file: SharedStr,
    /// Source code text.
    pub code: String,
    /// Source language of the file.
    pub language: SharedStr,
    /// Module system used for code generation.
    pub source_type: SharedStr,
    /// Options resolved for the hook/stage.
    pub options: crate::_types::plugin::hooks::JsParserHookOptions,
}

impl HookCall for ParseCall {
    type Payload = JsParseArgs;
    type Return = Option<JsParseOutput>;

    fn into_payload(self) -> Result<(JsPluginContext, Self::Payload)> {
        let context: JsPluginContext = plugin_context(
            self.cwd,
            self.file,
            self.language,
            self.source_type,
        )?;

        let payload: JsParseArgs =
            JsParseArgs { code: self.code, options: self.options };

        Ok((context, payload))
    }
}

/// TSFN bridging the `parse` hook.
pub type ParseTsfn =
    Tsfn<ParseCall, parse::Args, Either<Promise<parse::Return>, parse::Return>>;

/// Worker-side call data for the `prepare` / `finalize` observers.
pub struct ObserveCall {
    /// Working directory used to resolve relative paths.
    pub cwd: SharedStr,
    /// The file being parsed/generated.
    pub file: SharedStr,
    /// Source language of the file.
    pub language: SharedStr,
    /// Module system used for code generation.
    pub source_type: SharedStr,
    /// The AST as a JSON string.
    pub ast_json: String,
    /// Source text the program's spans anchor to.
    pub code: String,
    /// The input source map, when the AST carries one.
    pub map: Option<JsSourceMap>,
}

impl HookCall for ObserveCall {
    type Payload = JsAstArgs;
    type Return = Undefined;

    fn into_payload(self) -> Result<(JsPluginContext, Self::Payload)> {
        let context: JsPluginContext = plugin_context(
            self.cwd,
            self.file,
            self.language,
            self.source_type,
        )?;

        let payload: JsAstArgs = JsAstArgs {
            ast_json: self.ast_json,
            code: self.code,
            map: self.map,
        };

        Ok((context, payload))
    }
}

/// TSFN bridging the `prepare` hook.
pub type PrepareTsfn = Tsfn<
    ObserveCall,
    prepare::Args,
    Either<Promise<prepare::Return>, prepare::Return>,
>;

/// TSFN bridging the `finalize` hook.
pub type FinalizeTsfn = Tsfn<
    ObserveCall,
    finalize::Args,
    Either<Promise<finalize::Return>, finalize::Return>,
>;

/// Worker-side call data for the `transform` hook.
pub struct TransformCall {
    /// Working directory used to resolve relative paths.
    pub cwd: SharedStr,
    /// The file being parsed/generated.
    pub file: SharedStr,
    /// Source language of the file.
    pub language: SharedStr,
    /// Module system used for code generation.
    pub source_type: SharedStr,
    /// The AST as a JSON string.
    pub ast_json: String,
    /// Source text the program's spans anchor to.
    pub code: String,
    /// The input source map, when the AST carries one.
    pub map: Option<JsSourceMap>,
}

impl HookCall for TransformCall {
    type Payload = JsAstArgs;
    type Return = Option<JsTransformOutput>;

    fn into_payload(self) -> Result<(JsPluginContext, Self::Payload)> {
        let context: JsPluginContext = plugin_context(
            self.cwd,
            self.file,
            self.language,
            self.source_type,
        )?;

        let payload: JsAstArgs = JsAstArgs {
            ast_json: self.ast_json,
            code: self.code,
            map: self.map,
        };

        Ok((context, payload))
    }
}

/// TSFN bridging the `transform` hook.
pub type TransformTsfn = Tsfn<
    TransformCall,
    transform::Args,
    Either<Promise<transform::Return>, transform::Return>,
>;

/// Worker-side call data for the `generate` hook.
pub struct GenerateCall {
    /// Working directory used to resolve relative paths.
    pub cwd: SharedStr,
    /// The file being parsed/generated.
    pub file: SharedStr,
    /// Source language of the file.
    pub language: SharedStr,
    /// Module system used for code generation.
    pub source_type: SharedStr,
    /// The AST as a normalized object.
    pub ast: JsAst,
    /// Options resolved for the hook/stage.
    pub options: crate::_types::plugin::hooks::JsGeneratorHookOptions,
}

impl HookCall for GenerateCall {
    type Payload = JsGenerateArgs;
    type Return = Option<JsCodeOutput>;

    fn into_payload(self) -> Result<(JsPluginContext, Self::Payload)> {
        let context: JsPluginContext = plugin_context(
            self.cwd,
            self.file,
            self.language,
            self.source_type,
        )?;

        let payload: JsGenerateArgs =
            JsGenerateArgs { ast: self.ast, options: self.options };

        Ok((context, payload))
    }
}

/// TSFN bridging the `generate` hook.
pub type GenerateTsfn = Tsfn<
    GenerateCall,
    generate::Args,
    Either<Promise<generate::Return>, generate::Return>,
>;

/// Worker-side call data for the `end` hook: the rendered compile error, when
/// the compile failed.
pub struct EndCall {
    /// Working directory used to resolve relative paths.
    pub cwd: SharedStr,
    /// The file being parsed/generated.
    pub file: SharedStr,
    /// Source language of the file.
    pub language: SharedStr,
    /// Module system used for code generation.
    pub source_type: SharedStr,
    /// Error raised by an earlier hook.
    pub error: Option<String>,
}

impl HookCall for EndCall {
    type Payload = JsEndArgs;
    type Return = Undefined;

    fn into_payload(self) -> Result<(JsPluginContext, Self::Payload)> {
        let context: JsPluginContext = plugin_context(
            self.cwd,
            self.file,
            self.language,
            self.source_type,
        )?;

        let payload: JsEndArgs = JsEndArgs { error: self.error };

        Ok((context, payload))
    }
}

/// TSFN bridging the `end` hook.
pub type EndTsfn =
    Tsfn<EndCall, end::Args, Either<Promise<end::Return>, end::Return>>;

/// A TSFN bridging one ctx-bearing JS hook: the worker-side call data `C`,
/// the `FnCtx<C::Payload>` JS payload, and the sync/async `Either` return.
pub type CtxTsfn<C> = Tsfn<
    C,
    FnCtx<<C as HookCall>::Payload>,
    Either<Promise<<C as HookCall>::Return>, <C as HookCall>::Return>,
>;

/// Scan a ctx-bearing hook slot on a JS plugin object and bridge its function
/// into a TSFN.
pub fn scan_hook<C>(
    object: &Object<'static>,
    name: &str,
) -> Result<Option<CtxTsfn<C>>>
where
    C: HookCall,
{
    let Some(function) = object.get::<Function<
        '_,
        FnCtx<C::Payload>,
        Either<Promise<C::Return>, C::Return>,
    >>(name)?
    else {
        return Ok(None);
    };

    let tsfn: CtxTsfn<C> = function
        .build_threadsafe_function::<C>()
        .build_callback(move |callback: ThreadsafeCallContext<C>| {
            let (context, payload): (JsPluginContext, C::Payload) =
                callback.value.into_payload()?;

            Ok(FnCtx(context, payload))
        })?;

    Ok(Some(tsfn))
}

/// Scan the `options` hook slot on a JS plugin object and bridge its function
/// into a TSFN. The `options` hook runs before any context exists, so its
/// payload is the `OptionsCall` data itself (carrying the rooted plugin array
/// instead of a context).
pub fn scan_options(object: &Object<'static>) -> Result<Option<OptionsTsfn>> {
    let Some(function) = object.get::<Function<
        '_,
        OptionsCall,
        crate::plugin::pluginable::OptionsReturn,
    >>("options")?
    else {
        return Ok(None);
    };

    let tsfn: OptionsTsfn =
        function.build_threadsafe_function::<OptionsCall>().build_callback(
            |callback: ThreadsafeCallContext<OptionsCall>| Ok(callback.value),
        )?;

    Ok(Some(tsfn))
}

/// Scan every hook slot of a JS plugin object, bridging present hook
/// functions into TSFNs.
///
/// Returns `(options, start, parse, prepare, transform, finalize, generate,
/// end)`, in pipeline execution order.
macro_rules! hook_scan {
    ($object:expr) => {{
        let options: Option<crate::plugin::hooks::OptionsTsfn> =
            crate::plugin::hooks::scan_options($object)?;

        let start: Option<crate::plugin::hooks::StartTsfn> =
            crate::plugin::hooks::scan_hook::<crate::plugin::hooks::StartCall>(
                $object, "start",
            )?;

        let parse: Option<crate::plugin::hooks::ParseTsfn> =
            crate::plugin::hooks::scan_hook::<crate::plugin::hooks::ParseCall>(
                $object, "parse",
            )?;

        let prepare: Option<crate::plugin::hooks::PrepareTsfn> =
            crate::plugin::hooks::scan_hook::<crate::plugin::hooks::ObserveCall>(
                $object, "prepare",
            )?;

        let transform: Option<crate::plugin::hooks::TransformTsfn> =
            crate::plugin::hooks::scan_hook::<
                crate::plugin::hooks::TransformCall,
            >($object, "transform")?;

        let finalize: Option<crate::plugin::hooks::FinalizeTsfn> =
            crate::plugin::hooks::scan_hook::<crate::plugin::hooks::ObserveCall>(
                $object, "finalize",
            )?;

        let generate: Option<crate::plugin::hooks::GenerateTsfn> =
            crate::plugin::hooks::scan_hook::<
                crate::plugin::hooks::GenerateCall,
            >($object, "generate")?;

        let end: Option<crate::plugin::hooks::EndTsfn> =
            crate::plugin::hooks::scan_hook::<crate::plugin::hooks::EndCall>(
                $object, "end",
            )?;

        (options, start, parse, prepare, transform, finalize, generate, end)
    }};
}

pub(crate) use hook_scan;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hook_slots_are_sequential() {
        assert_eq!(options::SLOT, 0);
        assert_eq!(start::SLOT, 1);
        assert_eq!(parse::SLOT, 2);
        assert_eq!(prepare::SLOT, 3);
        assert_eq!(transform::SLOT, 4);
        assert_eq!(finalize::SLOT, 5);
        assert_eq!(generate::SLOT, 6);
        assert_eq!(end::SLOT, 7);
    }

    #[test]
    fn test_shared_str_from_borrow_and_as_str() {
        let shared: SharedStr = SharedStr::new("index.ts");
        assert_eq!(shared.as_str(), "index.ts");
    }

    #[test]
    fn test_shared_str_clone_shares_allocation() {
        let shared: SharedStr = SharedStr::new("const a = 1;");

        let clone: SharedStr = shared.clone();

        assert_eq!(clone.as_str(), "const a = 1;");

        let original: SharedStr = shared;

        assert_eq!(original.as_str(), "const a = 1;");
        assert_eq!(clone.as_str(), "const a = 1;");
    }

    #[test]
    fn test_shared_str_from_impl() {
        let value: &str = "console.log(1);";

        let shared: SharedStr = SharedStr::from(value);

        assert_eq!(shared.as_str(), value);
    }

    #[test]
    fn test_shared_str_debug() {
        let shared: SharedStr = SharedStr::new("x");
        assert_eq!(format!("{shared:?}"), r#"SharedStr("x")"#);
    }

    #[test]
    fn test_start_call_payload_carries_resolved_options() {
        let call: StartCall = StartCall {
            cwd: SharedStr::new("/repo"),
            file: SharedStr::new("index.ts"),
            language: SharedStr::new("ts"),
            source_type: SharedStr::new("module"),
            options: JsResolvedOptions {
                cwd: String::from("/repo"),
                file: String::from("index.ts"),
                language: String::from("ts"),
                source_type: String::from("module"),
                output: crate::_types::plugin::hooks::JsResolvedOutputOptions {
                    file: String::from("dist/index.js"),
                },
                source_map: serde_json::json!({
                    "input": "auto",
                    "output": "both",
                }),
                plugins: vec![String::from("observe")],
            },
        };

        let (_context, payload) = call.into_payload().expect("payload builds");

        assert_eq!(payload.options.file, "index.ts");
        assert_eq!(payload.options.cwd, "/repo");
        assert_eq!(payload.options.plugins, vec![String::from("observe")]);
        assert_eq!(
            payload.options.source_map,
            serde_json::json!({ "input": "auto", "output": "both" })
        );
    }
}
