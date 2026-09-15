use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use napi::bindgen_prelude::{
    FromNapiValue, Function, JsValue, JsValuesTupleIntoVec, Object, ObjectRef,
    Promise, ToNapiValue, TypeName, Undefined, ValidateNapiValue,
};
use napi::sys;
use napi::threadsafe_function::{ThreadsafeCallContext, ThreadsafeFunction};
use napi::{Either, Env, Error, Result, Status};

use crate::_types::plugin::context::JsPluginContext;
use crate::_types::plugin::hooks::{
    JsStageArgs, JsTransformArgs, JsTransformOutput,
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

macro_rules! for_each_hook_slot {
    ($macro_name:ident $(, $extra:tt)*) => {
        $macro_name! {
            $($extra,)*
            {
                (options, Options, crate::_types::options::JsOptions, Option<crate::_types::plugin::hooks::JsOptionsOutput>, 0),
                (pre, Pre, crate::plugin::hooks::FnCtx<crate::_types::plugin::hooks::JsStageArgs>, napi::bindgen_prelude::Undefined, 1),
                (transform, Transform, crate::plugin::hooks::FnCtx<crate::_types::plugin::hooks::JsTransformArgs>, Option<crate::_types::plugin::hooks::JsTransformOutput>, 2),
                (post, Post, crate::plugin::hooks::FnCtx<crate::_types::plugin::hooks::JsStageArgs>, napi::bindgen_prelude::Undefined, 3),
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

/// Worker-side call data for the `options` hook.
pub struct OptionsCall {
    /// Current working directory.
    pub cwd: String,
    /// The file being compiled.
    pub file: String,
    /// The code being compiled.
    pub code: String,
    /// Rooted JS array holding every raw plugin object.
    pub plugins: SharedRef,
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

        let plugins: Object<'static> = val.plugins.get(&env)?;
        object.set("plugins", JsValue::raw(&plugins))?;

        Ok(JsValue::raw(&object))
    }
}

/// TSFN bridging the `options` hook. Its payload is emitted by
/// [`OptionsCall`] in the `JsOptions` shape (the slot table's `Args`); the
/// `plugins` property is the rooted JS array of every raw plugin object.
pub type OptionsTsfn = Tsfn<
    OptionsCall,
    OptionsCall,
    Either<Promise<options::Return>, options::Return>,
>;

/// A worker-side ctx-bearing hook call.
pub trait HookCall: 'static {
    /// JS args payload accompanying the context.
    type Payload: ToNapiValue;

    /// JS return type of the hook (before the sync/async `Either`).
    type Return: FromNapiValue + TypeName + ValidateNapiValue + 'static;

    /// Build the JS args payload for this call.
    fn payload(&self) -> Self::Payload;

    /// Current working directory.
    fn cwd(&self) -> &str;

    /// The file being compiled.
    fn file(&self) -> &str;

    /// The code being compiled.
    fn code(&self) -> &str;
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

/// Worker-side call data for the `pre` / `post` hooks.
pub struct StageCall {
    /// Current working directory.
    pub cwd: String,
    /// The file being compiled.
    pub file: String,
    /// The code being compiled.
    pub code: String,
}

impl HookCall for StageCall {
    type Payload = JsStageArgs;
    type Return = Undefined;

    fn payload(&self) -> Self::Payload {
        JsStageArgs { file: self.file.clone(), code: self.code.clone() }
    }

    fn cwd(&self) -> &str {
        &self.cwd
    }

    fn file(&self) -> &str {
        &self.file
    }

    fn code(&self) -> &str {
        &self.code
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
    pub cwd: String,
    /// The file being compiled.
    pub file: String,
    /// The original code, anchoring the read-back AST spans.
    pub code: String,
    /// The AST as a JSON string.
    pub ast_json: String,
}

impl HookCall for TransformCall {
    type Payload = JsTransformArgs;
    type Return = Option<JsTransformOutput>;

    fn payload(&self) -> Self::Payload {
        JsTransformArgs {
            file: self.file.clone(),
            ast_json: self.ast_json.clone(),
        }
    }

    fn cwd(&self) -> &str {
        &self.cwd
    }

    fn file(&self) -> &str {
        &self.file
    }

    fn code(&self) -> &str {
        &self.code
    }
}

/// TSFN bridging the `transform` hook.
pub type TransformTsfn = Tsfn<
    TransformCall,
    transform::Args,
    Either<Promise<transform::Return>, transform::Return>,
>;

/// Build the JS-facing plugin context for a hook call.
pub fn plugin_context(
    cwd: &str,
    file: &str,
    code: &str,
    metadata: &SharedRef,
    env: &Env,
) -> Result<JsPluginContext> {
    Ok(JsPluginContext {
        cwd: cwd.to_string(),
        file: file.to_string(),
        code: code.to_string(),
        metadata: metadata.get(env)?,
    })
}

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
    metadata: &SharedRef,
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

    let metadata: SharedRef = metadata.clone();
    let tsfn: CtxTsfn<C> = function
        .build_threadsafe_function::<C>()
        .build_callback(move |callback: ThreadsafeCallContext<C>| {
            let payload: C::Payload = callback.value.payload();
            let context: JsPluginContext = plugin_context(
                callback.value.cwd(),
                callback.value.file(),
                callback.value.code(),
                &metadata,
                &callback.env,
            )?;

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
        Either<Promise<options::Return>, options::Return>,
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
/// Returns `(options, pre, transform, post)`.
macro_rules! hook_scan {
    ($object:expr, $metadata:expr) => {{
        let options: Option<crate::plugin::hooks::OptionsTsfn> =
            crate::plugin::hooks::scan_options($object)?;

        let pre: Option<crate::plugin::hooks::PreTsfn> =
            crate::plugin::hooks::scan_hook::<crate::plugin::hooks::StageCall>(
                $object, "pre", $metadata,
            )?;

        let transform: Option<crate::plugin::hooks::TransformTsfn> =
            crate::plugin::hooks::scan_hook::<
                crate::plugin::hooks::TransformCall,
            >($object, "transform", $metadata)?;

        let post: Option<crate::plugin::hooks::PostTsfn> =
            crate::plugin::hooks::scan_hook::<crate::plugin::hooks::StageCall>(
                $object, "post", $metadata,
            )?;

        (options, pre, transform, post)
    }};
}

pub(crate) use hook_scan;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hook_slots_are_sequential() {
        assert_eq!(options::SLOT, 0);
        assert_eq!(pre::SLOT, 1);
        assert_eq!(transform::SLOT, 2);
        assert_eq!(post::SLOT, 3);
    }
}
