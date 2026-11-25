use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use napi::bindgen_prelude::{
    Array, Either, FromNapiValue, Function, JsValue, JsValuesTupleIntoVec,
    Object, ObjectRef, Promise, ToNapiValue, TypeName, Undefined,
    ValidateNapiValue,
};
use napi::sys;
use napi::threadsafe_function::{ThreadsafeCallContext, ThreadsafeFunction};
use napi::{Env, Error, Result, Status};

use telarel_plugin::SharedPluginable;

use crate::_types::ast::JsAst;
use crate::_types::plugin::context::JsPluginContext;
use crate::_types::plugin::hooks::{
    JsAstArgs, JsCodeOutput, JsEndArgs, JsGenerateArgs, JsParseArgs,
    JsParseOutput, JsResolvedOptions, JsStartArgs, JsTransformBatchArgs,
    JsTransformOutput,
};
use crate::_types::sourcemap::JsSourceMap;

/// A TSFN bridging one JS hook.
///
/// - `Data`: worker-side call data moved to the JS thread.
/// - `Payload`: JS value or values the hook receives.
/// - `Return`: JS value the hook returns directly (sync) or resolves (async).
///   The hook aliases wrap it in an `Either` over `Promise<Return>`.
///
/// `CalleeHandled = false` passes hook payloads to the JS function as-is. The
/// `true` variant would prepend a Node-style error argument and shift every
/// hook parameter by one. The bridge still captures a JS throw inside the
/// hook. `call_async_catch` uses the `WithCallback` call variant, so the
/// exception reaches the awaiting worker as `Err(napi::Error)` with status
/// `PendingException`. The compile promise then rejects with the original
/// message instead of letting the error escape as a fatal exception.
pub type Tsfn<Data, Payload, Return> =
    ThreadsafeFunction<Data, Return, Payload, napi::Status, false, false, 0>;

/// A cheap-to-clone shared string.
///
/// Clones share one `Arc` allocation. The `ToNapiValue` implementation
/// serializes it as a plain UTF-8 string, so hook payload shapes are unchanged.
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
        $( // Declarative slot metadata. The `options` hook's `Args` is the
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
/// [`ObjectRef`] is `Send` but not `Sync`. The bridge dereferences the
/// reference (through `get_value`) only on the JS thread, so sharing the
/// immutable handle across threads is sound.
///
/// [`SharedRef::release`] releases (unroots) the inner reference, and the
/// `compile` task calls it once the compile finishes. It is idempotent and
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

    /// An inert placeholder reference for tests, with no rooted object. It
    /// exercises the bag and release-list bookkeeping without a JS runtime.
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

    /// Release the rooted reference. The method is idempotent. Clones that
    /// remain become inert, so `get` errors instead of touching the released
    /// reference.
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

/// Release every reference, even after one release fails.
///
/// This keeps the first-error path from leaking any reference.
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

/// One entry in the options-payload descriptor view.
///
/// The reference shares its root with the release list. `id` is the JS
/// wrapper's internal plugin identity when the descriptor carries one. An
/// `options` patch uses it to avoid repeating a descriptor the view already
/// holds.
pub struct BagDescriptor {
    /// The JS wrapper's `__PLUGIN_ID__`, when the descriptor carries one.
    pub(crate) id: Option<u32>,
    /// The rooted descriptor reference.
    pub(crate) reference: SharedRef,
}

#[cfg(test)]
impl BagDescriptor {
    /// An inert entry for view-bookkeeping tests.
    pub(crate) fn stub(id: Option<u32>) -> Self {
        Self { id, reference: SharedRef::stub() }
    }
}

/// A dynamic, per-compile registry of rooted JS object references.
///
/// The same `Arc` handle reaches every site that needs one of its three roles:
///
/// - `refs`: the release list. It starts with the entry-time references: the
///   plugin array and each plugin object. The `options` hook appends late
///   plugin references as the bridge wraps and registers them, before the
///   driver is built. `FunctionTask::finally` drains the list once the call
///   settles, so mid-call additions are still released.
/// - `bag_descriptors`: the options-payload descriptor view, one entry per
///   plugin in the settled `OptionsArgs` in bag order. The entry bridge seeds
///   it in registration order, duplicates included. Each `options` patch
///   appends the descriptors it added, so later hooks see the full settled
///   list. Each entry is a `SharedRef` clone that shares its root with the
///   release list. The table never owns or releases a reference.
///   [`RefList::seed_bag`] replaces the view. [`RefList::append_bag`] extends
///   it without repeating an id.
/// - `bridged_plugins`: the adapter memo. It maps the internal plugin id from
///   the JS wrapper to the already-bridged adapter for the current compile. An
///   echoed descriptor therefore reuses the same `SharedPluginable`.
#[derive(Default)]
pub struct RefList {
    /// Rooted references drained in `FunctionTask::finally`.
    refs: Mutex<Vec<SharedRef>>,
    bag_descriptors: Mutex<Vec<BagDescriptor>>,
    bridged_plugins: Mutex<HashMap<u32, SharedPluginable>>,
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

    /// Clone the descriptor view for the settled options bag, one entry per
    /// plugin in bag order.
    pub fn bag_snapshot(&self) -> Vec<SharedRef> {
        self.bag_descriptors
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .map(|entry: &BagDescriptor| entry.reference.clone())
            .collect()
    }

    /// The already-bridged adapter memoized for `id`, if any.
    pub fn bridged_plugin(
        &self,
        id: u32,
    ) -> Option<SharedPluginable> {
        self.bridged_plugins
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&id)
            .cloned()
    }

    /// Memoize a bridged adapter under the JS wrapper's plugin id.
    pub fn remember_bridged(
        &self,
        id: u32,
        adapter: SharedPluginable,
    ) {
        self.bridged_plugins
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(id, adapter);
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

    /// Replace the descriptor view with the entry descriptors.
    ///
    /// The entry list keeps duplicate entries of one instance, so this seed
    /// does not dedupe by id. Only the entry bridge calls it. On a partial
    /// unmarshal failure the table stays untouched: the compile aborts and the
    /// release list still drains every rooted descriptor.
    ///
    /// Rust-native plugins that return bags do not flow through this NAPI
    /// surface, so only the entry bag and `options`-returned patches matter
    /// here.
    pub(crate) fn seed_bag(
        &self,
        descriptors: Vec<BagDescriptor>,
    ) {
        let mut bag: MutexGuard<'_, Vec<BagDescriptor>> =
            self.bag_descriptors.lock().unwrap_or_else(PoisonError::into_inner);

        *bag = descriptors;
    }

    /// Append the descriptors an `options` patch added.
    ///
    /// A descriptor whose id already appears in the view is skipped, so an
    /// echoed bag or a repeated descriptor cannot duplicate the view.
    /// A descriptor without an id appends as a new entry. Patch order is
    /// preserved. On a partial unmarshal failure the table stays untouched:
    /// the compile aborts and the release list still drains every rooted
    /// descriptor.
    pub(crate) fn append_bag(
        &self,
        descriptors: Vec<BagDescriptor>,
    ) {
        let mut bag: MutexGuard<'_, Vec<BagDescriptor>> =
            self.bag_descriptors.lock().unwrap_or_else(PoisonError::into_inner);

        for descriptor in descriptors {
            let is_present: bool = descriptor.id.is_some_and(|id| {
                bag.iter().any(|present: &BagDescriptor| present.id == Some(id))
            });

            if is_present {
                continue;
            }

            bag.push(descriptor);
        }
    }

    /// Release every reference on the release list and clear the adapter memo.
    ///
    /// The first release error wins. A memoized adapter holds this `RefList`,
    /// so the method clears the memo before it releases references. A failed
    /// release must not leave the `RefList -> JsPlugin -> RefList` cycle
    /// behind.
    pub fn release(
        &self,
        env: &Env,
    ) -> Result<()> {
        let refs: Vec<SharedRef> = self.refs();

        // A memoized adapter holds this `RefList`. Clear the memo before
        // releasing references so a failed release does not leave the
        // `RefList -> JsPlugin -> RefList` cycle behind.
        self.bridged_plugins
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();

        release_refs(env, &refs)
    }
}

#[cfg(test)]
impl RefList {
    /// The ids of the descriptor view, for bookkeeping tests.
    pub(crate) fn bag_ids(&self) -> Vec<Option<u32>> {
        self.bag_descriptors
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .map(|entry: &BagDescriptor| entry.id)
            .collect()
    }
}

/// Worker-side call data for the `options` hook.
pub struct OptionsCall {
    /// Working directory used to resolve relative paths. Absent when unset.
    pub cwd: Option<SharedStr>,
    /// The file being parsed or generated. Absent when unset.
    pub file: Option<SharedStr>,
    /// Source language of the file. The wire value is `""` when unspecified.
    pub language: SharedStr,
    /// Module system used for code generation. The wire value is `""` when
    /// unspecified.
    pub source_type: SharedStr,
    /// The parser-stage options requested so far, as a JS object. `None`
    /// means unspecified, and Telarel omits it from the payload.
    pub parser: Option<serde_json::Value>,
    /// The generator-stage options requested so far, as a JS object. `None`
    /// means unspecified, and Telarel omits it from the payload.
    pub generator: Option<serde_json::Value>,
    /// The output code options requested so far, as a JS object. `None`
    /// means unspecified, and Telarel omits it from the payload.
    pub output: Option<serde_json::Value>,
    /// The source-map inputs and outputs requested so far, as the JS union.
    /// `None` means unspecified, and Telarel omits it from the payload.
    pub source_map: Option<serde_json::Value>,
    /// The settled plugin descriptors of the options bag, snapshotted at call
    /// time. The view starts with the entry descriptors and grows by each
    /// patch's additions, so every later hook sees the full settled list.
    pub plugins: Vec<SharedRef>,
}

impl ToNapiValue for OptionsCall {
    unsafe fn to_napi_value(
        env: sys::napi_env,
        val: Self,
    ) -> Result<sys::napi_value> {
        let env: Env = Env::from_raw(env);

        let mut object: Object<'static> = Object::new(&env)?;

        // `cwd` and `file` are omitted when unset so an echoed bag keeps
        // the resolution defaults. `language` and `sourceType` always ship;
        // an empty label reads as unset on the JS side.
        if let Some(cwd) = val.cwd {
            object.set("cwd", cwd)?;
        }

        if let Some(file) = val.file {
            object.set("file", file)?;
        }

        object.set("language", val.language)?;
        object.set("sourceType", val.source_type)?;

        // An omitted stage group means "unspecified" on the wire. Telarel
        // transmits only a requested group.
        if let Some(parser) = val.parser {
            object.set("parser", parser)?;
        }

        if let Some(generator) = val.generator {
            object.set("generator", generator)?;
        }

        // An omitted `output` means "unspecified" on the wire. Telarel
        // transmits only a requested output.
        if let Some(output) = val.output {
            object.set("output", output)?;
        }

        // An omitted `sourceMap` means "unspecified" on the wire. Telarel
        // transmits only a requested map.
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

/// TSFN bridging the `options` hook.
///
/// [`OptionsCall`] (the slot table's `Args`) emits its payload, whose
/// `plugins` property is the settled plugin descriptors of the options bag.
/// The raw `options` hook is async and returns a promise that carries
/// `Option<JsOptionsOutput>`. The TSFN delivers that promise raw. See
/// [`crate::plugin::pluginable::OptionsReturn`].
pub type OptionsTsfn =
    Tsfn<OptionsCall, OptionsCall, crate::plugin::pluginable::OptionsReturn>;

/// A worker-side hook call that carries a plugin context.
pub trait HookCall: 'static {
    /// JS args payload that accompanies the context.
    type Payload: ToNapiValue;

    /// JS return type of the hook, before the sync or async `Either`.
    type Return: FromNapiValue + TypeName + ValidateNapiValue + 'static;

    /// Consume the call data into the JS-facing plugin context and args
    /// payload.
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

/// A hook payload: the plugin context, then the hook args.
pub struct FnCtx<A>(
    /// Plugin context passed as the first hook argument.
    pub JsPluginContext,
    /// Hook args passed as the second hook argument.
    pub A,
);

impl<A: ToNapiValue> JsValuesTupleIntoVec for FnCtx<A> {
    // `env` is a raw napi handle that the JS thread owns. The trait method is
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

/// Worker-side call data for the `start` hook: the resolved, read-only
/// options with the settled plugin-name list.
pub struct StartCall {
    /// Working directory used to resolve relative paths.
    pub cwd: SharedStr,
    /// The file being parsed or generated.
    pub file: SharedStr,
    /// Source language of the file.
    pub language: SharedStr,
    /// Module system used for code generation.
    pub source_type: SharedStr,
    /// Options resolved for the hook or stage.
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

/// Worker-side call data for the `parse` hook: the code to parse and the
/// resolved parser options.
pub struct ParseCall {
    /// Working directory used to resolve relative paths.
    pub cwd: SharedStr,
    /// The file being parsed or generated.
    pub file: SharedStr,
    /// Source code text.
    pub code: String,
    /// Source language of the file.
    pub language: SharedStr,
    /// Module system used for code generation.
    pub source_type: SharedStr,
    /// Options resolved for the hook or stage.
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

/// Worker-side call data for the `prepare` and `finalize` observers.
pub struct ObserveCall {
    /// Working directory used to resolve relative paths.
    pub cwd: SharedStr,
    /// The file being parsed or generated.
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
    /// Grammar of the AST.
    pub ast_language: String,
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
            language: self.ast_language,
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
    /// The file being parsed or generated.
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
    /// Grammar of the AST the hook receives.
    pub ast_language: String,
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
            language: self.ast_language,
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

/// Worker-side call data for a batched `transform` run.
pub struct TransformBatchCall {
    /// Working directory used to resolve relative paths.
    pub cwd: SharedStr,
    /// The file being parsed or generated.
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
    /// Grammar of the AST the batch receives.
    pub ast_language: String,
    /// The raw plugin descriptors for this run, in driver order.
    pub plugins: Vec<SharedRef>,
}

impl HookCall for TransformBatchCall {
    type Payload = JsTransformBatchArgs;
    type Return = Option<JsTransformOutput>;

    fn into_payload(self) -> Result<(JsPluginContext, Self::Payload)> {
        let context: JsPluginContext = plugin_context(
            self.cwd,
            self.file,
            self.language,
            self.source_type,
        )?;

        let payload: JsTransformBatchArgs = JsTransformBatchArgs {
            ast_json: self.ast_json,
            code: self.code,
            map: self.map,
            language: self.ast_language,
            plugins: self.plugins,
        };

        Ok((context, payload))
    }
}

/// TSFN bridging one batched `transform` run.
pub type TransformBatchTsfn = CtxTsfn<TransformBatchCall>;

/// Worker-side call data for the `generate` hook.
pub struct GenerateCall {
    /// Working directory used to resolve relative paths.
    pub cwd: SharedStr,
    /// The file being parsed or generated.
    pub file: SharedStr,
    /// Source language of the file.
    pub language: SharedStr,
    /// Module system used for code generation.
    pub source_type: SharedStr,
    /// The AST as a normalized object.
    pub ast: JsAst,
    /// Options resolved for the hook or stage.
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
    /// The file being parsed or generated.
    pub file: SharedStr,
    /// Source language of the file.
    pub language: SharedStr,
    /// Module system used for code generation.
    pub source_type: SharedStr,
    /// An error that an earlier hook raised.
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

/// A TSFN bridging one JS hook that carries a context. It carries the
/// worker-side call data `C`, the `FnCtx<C::Payload>` JS payload, and the sync
/// or async `Either` return.
pub type CtxTsfn<C> = Tsfn<
    C,
    FnCtx<<C as HookCall>::Payload>,
    Either<Promise<<C as HookCall>::Return>, <C as HookCall>::Return>,
>;

/// Scan a hook slot that carries a context on a JS plugin object. Bridge the
/// present function into a TSFN.
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
/// into a TSFN.
///
/// The `options` hook runs before any context exists, so its payload is the
/// `OptionsCall` data itself. That payload carries the rooted plugin array
/// instead of a context.
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

/// Scan every hook slot of a JS plugin object, and bridge each present hook
/// function into a TSFN.
///
/// The macro returns `(options, start, parse, prepare, transform, finalize,
/// generate, end)`, in pipeline execution order.
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

    fn test_adapter() -> SharedPluginable {
        crate::plugin::builtin::to_builtin_plugin("builtin:transform", None)
            .expect("builtin:transform dispatches")
    }

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
    fn test_bridged_plugin_memo_round_trip() {
        let refs: RefList = RefList::new();
        let adapter: SharedPluginable = test_adapter();

        refs.remember_bridged(7, Arc::clone(&adapter));

        let found: SharedPluginable =
            refs.bridged_plugin(7).expect("memoized adapter is returned");

        assert!(Arc::ptr_eq(&adapter, &found));
        assert!(refs.bridged_plugin(8).is_none());
    }

    #[test]
    fn test_bag_seed_keeps_duplicate_instance_ids() {
        let refs: RefList = RefList::new();

        refs.seed_bag(vec![
            BagDescriptor::stub(Some(1)),
            BagDescriptor::stub(Some(1)),
        ]);

        assert_eq!(refs.bag_ids(), vec![Some(1), Some(1)]);
    }

    #[test]
    fn test_bag_append_skips_present_ids_and_keeps_patch_order() {
        let refs: RefList = RefList::new();

        refs.seed_bag(vec![
            BagDescriptor::stub(Some(1)),
            BagDescriptor::stub(Some(2)),
        ]);

        // The echoed id 2 is skipped. The new id and the two unmarked
        // descriptors append in patch order.
        refs.append_bag(vec![
            BagDescriptor::stub(Some(2)),
            BagDescriptor::stub(Some(3)),
            BagDescriptor::stub(None),
            BagDescriptor::stub(None),
        ]);

        assert_eq!(refs.bag_ids(), vec![Some(1), Some(2), Some(3), None, None]);
    }

    #[test]
    fn test_release_clears_bridged_plugin_memo() {
        let refs: RefList = RefList::new();
        // The release list is empty, so `release` never dereferences the env.
        let env: Env = napi::Env::from_raw(std::ptr::null_mut());

        refs.remember_bridged(7, test_adapter());

        refs.release(&env).expect("empty release list releases");

        assert!(refs.bridged_plugin(7).is_none());
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
                parser:
                    crate::_types::plugin::hooks::JsResolvedParserStageOptions {
                        preserve_parens: false,
                        allow_return_outside_function: false,
                        oxc: serde_json::json!({}),
                    },
                generator:
                    crate::_types::plugin::hooks::JsResolvedGeneratorStageOptions
                    {
                        single_quote: false,
                        ascii_only: false,
                        indent_style: String::from("space"),
                        indent_width: 2,
                        comments: true,
                        minify: false,
                        oxc: serde_json::json!({}),
                    },
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
