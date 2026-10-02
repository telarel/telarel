use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use napi::bindgen_prelude::{
    Array, Either, FromNapiValue, Function, JsValue, JsValuesTupleIntoVec,
    Object, ObjectRef, Promise, ToNapiValue, TypeName, Undefined,
    ValidateNapiValue,
};
use napi::sys;
use napi::threadsafe_function::{ThreadsafeCallContext, ThreadsafeFunction};
use napi::{Env, Error, Result, Status};

use crate::_types::plugin::context::JsPluginContext;
use crate::_types::plugin::hooks::{
    JsCompileEndArgs, JsCompileStartArgs, JsStageArgs, JsTransformArgs,
    JsTransformOutput,
};

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
    /// Allocate a new shared string.
    pub fn new(value: &str) -> Self {
        Self(Arc::from(value))
    }

    /// Borrow the underlying string.
    #[allow(dead_code)] // exercised by the unit tests below
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
                (options, Options, crate::_types::options::JsOptions, Option<crate::_types::plugin::hooks::JsOptionsOutput>, 0),
                (compile_start, CompileStart, crate::plugin::hooks::FnCtx<crate::_types::plugin::hooks::JsCompileStartArgs>, napi::bindgen_prelude::Undefined, 1),
                (pre, Pre, crate::plugin::hooks::FnCtx<crate::_types::plugin::hooks::JsStageArgs>, Option<crate::_types::plugin::hooks::JsStageOutput>, 2),
                (transform, Transform, crate::plugin::hooks::FnCtx<crate::_types::plugin::hooks::JsTransformArgs>, Option<crate::_types::plugin::hooks::JsTransformOutput>, 3),
                (post, Post, crate::plugin::hooks::FnCtx<crate::_types::plugin::hooks::JsStageArgs>, Option<crate::_types::plugin::hooks::JsStageOutput>, 4),
                (compile_end, CompileEnd, crate::plugin::hooks::FnCtx<crate::_types::plugin::hooks::JsCompileEndArgs>, napi::bindgen_prelude::Undefined, 5),
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
           // `JsOptions` shape emitted by `OptionsCall`.
            #[allow(dead_code)]
            pub mod $name {
            /// Slot index of the hook in the plugin slot table.
            pub const SLOT: usize = $slot;

            /// JS payload type of the hook.
            pub type Args = $args;

            /// JS return type of the hook (before the sync/async `Either`).
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
    /// Root a JS object.
    #[allow(clippy::arc_with_non_send_sync)] // see the type's docs
    pub fn new(object: &Object<'static>) -> Result<Self> {
        Ok(Self(Arc::new(Mutex::new(Some(object.create_ref()?)))))
    }

    /// Wrap an already-rooted reference.
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

    /// Materialize the rooted object.
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
///   [`crate::tasks::compile::CompileTask::finally`] drains the list once the
///   compile settles, so mid-compile additions are still released.
/// - `bag_descriptors` — a VIEW of the CURRENT bag's plugin descriptors, one
///   per plugin in the `OptionsArgs` being folded. Entries are
///   `SharedRef` clones that SHARE the same roots as the release list (the
///   table never owns or releases a reference); the view is replaced whole by
///   [`RefList::set_bag`] (entry seed, then each returned bag).
#[derive(Default)]
pub struct RefList {
    /// Rooted references drained in `CompileTask::finally`.
    refs: Mutex<Vec<SharedRef>>,
    /// The current bag's descriptor view; see the type's docs.
    bag_descriptors: Mutex<Vec<SharedRef>>,
}

unsafe impl Send for RefList {}

unsafe impl Sync for RefList {}

impl RefList {
    /// Create an empty list.
    pub fn new() -> Self {
        Self::default()
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

    /// Snapshot the release-list references, in insertion order.
    pub fn refs(&self) -> Vec<SharedRef> {
        self.refs.lock().unwrap_or_else(PoisonError::into_inner).clone()
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

    /// Clone the current bag's descriptor view (one entry per plugin, in
    /// bag order).
    pub fn bag_snapshot(&self) -> Vec<SharedRef> {
        self.bag_descriptors
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Release every reference on the release list; first release error wins.
    pub fn release(
        &self,
        env: &Env,
    ) -> Result<()> {
        let refs: Vec<SharedRef> = self.refs();

        release_refs(env, &refs)
    }

    /// The number of rooted references on the release list.
    pub fn len(&self) -> usize {
        self.refs.lock().unwrap_or_else(PoisonError::into_inner).len()
    }
}

/// Worker-side call data for the `options` hook.
pub struct OptionsCall {
    /// Current working directory.
    pub cwd: SharedStr,
    /// The file being compiled.
    pub file: SharedStr,
    /// The code being compiled.
    pub code: SharedStr,
    /// The grammar of the code.
    pub language: SharedStr,
    /// The module system of the code.
    pub source_type: SharedStr,
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

        object.set("cwd", val.cwd)?;
        object.set("file", val.file)?;
        object.set("code", val.code)?;
        object.set("language", val.language)?;
        object.set("sourceType", val.source_type)?;

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
/// [`OptionsCall`] in the `JsOptions` shape (the slot table's `Args`); the
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
    code: SharedStr,
    language: SharedStr,
    source_type: SharedStr,
) -> Result<JsPluginContext> {
    Ok(JsPluginContext { cwd, file, code, language, source_type })
}

/// A hook payload: the plugin context followed by the hook args.
pub struct FnCtx<A>(pub JsPluginContext, pub A);

impl<A: ToNapiValue> JsValuesTupleIntoVec for FnCtx<A> {
    fn into_vec(
        self,
        env: sys::napi_env,
    ) -> Result<Vec<sys::napi_value>> {
        Ok(vec![unsafe { ToNapiValue::to_napi_value(env, self.0)? }, unsafe {
            ToNapiValue::to_napi_value(env, self.1)?
        }])
    }
}

/// Worker-side call data for the `compileStart` hook: resolved, read-only
/// options with the settled plugin-name list.
pub struct CompileStartCall {
    /// Current working directory, resolved.
    pub cwd: SharedStr,
    /// The file being compiled.
    pub file: SharedStr,
    /// The code being compiled.
    pub code: SharedStr,
    /// The grammar of the code, resolved.
    pub language: SharedStr,
    /// The module system of the code, resolved.
    pub source_type: SharedStr,
    /// The settled plugin-name list, after the options fixpoint.
    pub plugins: Vec<String>,
}

impl HookCall for CompileStartCall {
    type Payload = JsCompileStartArgs;
    type Return = Undefined;

    fn into_payload(self) -> Result<(JsPluginContext, Self::Payload)> {
        let cwd: SharedStr = self.cwd;

        let file: SharedStr = self.file;

        let code: SharedStr = self.code;

        let language: SharedStr = self.language;

        let source_type: SharedStr = self.source_type;

        let context: JsPluginContext = plugin_context(
            cwd.clone(),
            file.clone(),
            code.clone(),
            language.clone(),
            source_type.clone(),
        )?;

        let payload: JsCompileStartArgs = JsCompileStartArgs {
            cwd,
            file,
            code,
            language,
            source_type,
            plugins: self.plugins,
        };

        Ok((context, payload))
    }
}

/// TSFN bridging the `compileStart` hook.
pub type CompileStartTsfn = Tsfn<
    CompileStartCall,
    compile_start::Args,
    Either<Promise<compile_start::Return>, compile_start::Return>,
>;

/// Worker-side call data for the `pre` / `post` hooks.
pub struct StageCall {
    /// Current working directory.
    pub cwd: SharedStr,
    /// The file being compiled.
    pub file: SharedStr,
    /// The code being compiled.
    pub code: SharedStr,
    /// The grammar of the code.
    pub language: SharedStr,
    /// The module system of the code.
    pub source_type: SharedStr,
}

impl HookCall for StageCall {
    type Payload = JsStageArgs;
    type Return = Option<crate::_types::plugin::hooks::JsStageOutput>;

    fn into_payload(self) -> Result<(JsPluginContext, Self::Payload)> {
        let context: JsPluginContext = plugin_context(
            self.cwd.clone(),
            self.file.clone(),
            self.code.clone(),
            self.language,
            self.source_type,
        )?;

        let payload: JsStageArgs = JsStageArgs { code: self.code };

        Ok((context, payload))
    }
}

/// TSFN bridging the `pre` hook.
pub type PreTsfn =
    Tsfn<StageCall, pre::Args, Either<Promise<pre::Return>, pre::Return>>;

/// TSFN bridging the `post` hook.
pub type PostTsfn =
    Tsfn<StageCall, post::Args, Either<Promise<post::Return>, post::Return>>;

/// Worker-side call data for the `transform` hook.
pub struct TransformCall {
    /// Current working directory.
    pub cwd: SharedStr,
    /// The file being compiled.
    pub file: SharedStr,
    /// The original code, anchoring the read-back AST spans.
    pub code: SharedStr,
    /// The grammar of the code.
    pub language: SharedStr,
    /// The module system of the code.
    pub source_type: SharedStr,
    /// The AST as a JSON string.
    pub ast_json: String,
}

impl HookCall for TransformCall {
    type Payload = JsTransformArgs;
    type Return = Option<JsTransformOutput>;

    fn into_payload(self) -> Result<(JsPluginContext, Self::Payload)> {
        let context: JsPluginContext = plugin_context(
            self.cwd.clone(),
            self.file.clone(),
            self.code.clone(),
            self.language,
            self.source_type,
        )?;

        let payload: JsTransformArgs =
            JsTransformArgs { ast_json: self.ast_json };

        Ok((context, payload))
    }
}

/// TSFN bridging the `transform` hook.
pub type TransformTsfn = Tsfn<
    TransformCall,
    transform::Args,
    Either<Promise<transform::Return>, transform::Return>,
>;

/// Worker-side call data for the `compileEnd` hook: the last-good output
/// (`err` unset on the success path).
pub struct CompileEndCall {
    /// Current working directory.
    pub cwd: SharedStr,
    /// The file being compiled.
    pub file: SharedStr,
    /// The original code.
    pub code: SharedStr,
    /// The grammar of the code.
    pub language: SharedStr,
    /// The module system of the code.
    pub source_type: SharedStr,
    /// The compiled code, or the last good code on error.
    pub compiled: SharedStr,
    /// The source map JSON, or the last good map on error; `None` = no map.
    pub map: Option<String>,
    /// The error message, when the compile failed.
    pub err: Option<String>,
}

impl HookCall for CompileEndCall {
    type Payload = JsCompileEndArgs;
    type Return = Undefined;

    fn into_payload(self) -> Result<(JsPluginContext, Self::Payload)> {
        let context: JsPluginContext = plugin_context(
            self.cwd.clone(),
            self.file.clone(),
            self.code.clone(),
            self.language,
            self.source_type,
        )?;

        let payload: JsCompileEndArgs = JsCompileEndArgs {
            code: self.compiled,
            map: self.map,
            err: self.err,
        };

        Ok((context, payload))
    }
}

/// TSFN bridging the `compileEnd` hook.
pub type CompileEndTsfn = Tsfn<
    CompileEndCall,
    compile_end::Args,
    Either<Promise<compile_end::Return>, compile_end::Return>,
>;

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
/// payload is the `OptionsCall` data itself (emitted in the `JsOptions` shape,
/// carrying the rooted plugin array instead of a context).
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
/// Returns `(options, compile_start, pre, transform, post, compile_end)`,
/// in pipeline execution order.
macro_rules! hook_scan {
    ($object:expr) => {{
        let options: Option<crate::plugin::hooks::OptionsTsfn> =
            crate::plugin::hooks::scan_options($object)?;

        let compile_start: Option<crate::plugin::hooks::CompileStartTsfn> =
            crate::plugin::hooks::scan_hook::<
                crate::plugin::hooks::CompileStartCall,
            >($object, "compileStart")?;

        let pre: Option<crate::plugin::hooks::PreTsfn> =
            crate::plugin::hooks::scan_hook::<crate::plugin::hooks::StageCall>(
                $object, "pre",
            )?;

        let transform: Option<crate::plugin::hooks::TransformTsfn> =
            crate::plugin::hooks::scan_hook::<
                crate::plugin::hooks::TransformCall,
            >($object, "transform")?;

        let post: Option<crate::plugin::hooks::PostTsfn> =
            crate::plugin::hooks::scan_hook::<crate::plugin::hooks::StageCall>(
                $object, "post",
            )?;

        let compile_end: Option<crate::plugin::hooks::CompileEndTsfn> =
            crate::plugin::hooks::scan_hook::<
                crate::plugin::hooks::CompileEndCall,
            >($object, "compileEnd")?;

        (options, compile_start, pre, transform, post, compile_end)
    }};
}

pub(crate) use hook_scan;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::hooks::SharedStr;

    #[test]
    fn test_hook_slots_are_sequential() {
        assert_eq!(options::SLOT, 0);
        assert_eq!(compile_start::SLOT, 1);
        assert_eq!(pre::SLOT, 2);
        assert_eq!(transform::SLOT, 3);
        assert_eq!(post::SLOT, 4);
        assert_eq!(compile_end::SLOT, 5);
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

        // Clones share the same Arc allocation: both see the same data, and
        // dropping the original does not affect the clone.
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
}
