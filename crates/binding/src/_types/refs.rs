use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use napi::bindgen_prelude::{JsValue, Object, ObjectRef, ToNapiValue};
use napi::sys;
use napi::{Env, Error, Result, Status};

use telarel_plugin::SharedPluginable;

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

unsafe impl Send for SharedRef {}

unsafe impl Sync for SharedRef {}

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

/// A per-compile registry of rooted JS references and plugin adapters.
///
/// The registry releases every rooted reference when the compile ends.
#[derive(Default)]
pub struct RefList {
    /// Rooted references drained in `FunctionTask::finally`.
    refs: Mutex<Vec<SharedRef>>,
    bag_descriptors: Mutex<Vec<BagDescriptor>>,
    bridged_plugins: Mutex<HashMap<u32, SharedPluginable>>,
}

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
    /// unmarshal failure the view stays untouched: the compile aborts and the
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
    /// preserved. On a partial unmarshal failure the view stays untouched:
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
    /// release must not retain the `RefList -> JsPlugin -> RefList` cycle.
    pub fn release(
        &self,
        env: &Env,
    ) -> Result<()> {
        let refs: Vec<SharedRef> = self.refs();

        // A memoized adapter holds this `RefList`. Clear the memo before
        // releasing references so a failed release does not retain the
        // `RefList -> JsPlugin -> RefList` cycle.
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

unsafe impl Send for RefList {}

unsafe impl Sync for RefList {}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_adapter() -> SharedPluginable {
        crate::plugin::builtin::to_builtin_plugin("builtin:transform", None)
            .expect("builtin:transform dispatches")
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
}
