use std::collections::VecDeque;
use std::sync::Arc;

use napi::bindgen_prelude::ObjectRef;
use napi::{Env, Result};

use telarel_plugin::SharedPluginable;

use crate::plugin::builtin::to_builtin_plugin;
use crate::plugin::hooks::{BagDescriptor, RefList, SharedRef};
use crate::plugin::pluginable::JsPlugin;

/// The error raised when a JS plugin object carries no `name`.
pub const NAME_REQUIRED: &str = "plugin `name` is required";

/// Internal descriptor key shared with the JS wrapper. This key must match
/// `PLUGIN_ID` in `packages/telarel/src/bridges/plugin.ts`.
const PLUGIN_ID: &str = "__PLUGIN_ID__";

/// Internal batch-runner key shared with the JS wrapper. This key must match
/// `TRANSFORM_BATCH` in `packages/telarel/src/bridges/plugin.ts`.
pub const TRANSFORM_BATCH_KEY: &str = "__TRANSFORM_BATCH__";

/// Bridge one raw descriptor for a JS plugin into a shared adapter.
///
/// The caller has already rooted the descriptor's reference. Builtin markers
/// dispatch to Rust before any hook scan, for entry and mid-fixpoint plugins
/// alike. Plain JS plugins become [`JsPlugin`], which retains `plugin_ref` so a
/// batch payload can carry the descriptor back to JS.
pub fn bridge_plugin(
    object: &napi::bindgen_prelude::Object<'static>,
    refs: &Arc<RefList>,
    plugin_ref: &SharedRef,
) -> Result<SharedPluginable> {
    let is_builtin: bool = object
        .get::<serde_json::Value>("__builtin")?
        .and_then(|value: serde_json::Value| value.as_bool())
        == Some(true);

    if is_builtin {
        let name: String = object.get::<String>("name")?.ok_or_else(|| {
            napi::Error::new(napi::Status::InvalidArg, NAME_REQUIRED.to_owned())
        })?;

        let options: Option<serde_json::Value> =
            object.get::<serde_json::Value>("options")?;

        let adapter: SharedPluginable = to_builtin_plugin(&name, options)?;

        return Ok(adapter);
    }

    let adapter: SharedPluginable =
        Arc::new(JsPlugin::from_object(object, refs, plugin_ref)?);

    Ok(adapter)
}

/// Owns the descriptors [`convert_plugins`] has not bridged yet.
///
/// The caller roots every descriptor, and `ObjectRef` does not unref itself on
/// drop. The guard unrefs each descriptor that conversion never takes, so an
/// early return cannot leak a root. A descriptor that conversion takes moves
/// onto the `RefList`. The list owns its release from then on, so the guard
/// and the list never release the same root.
struct DescriptorGuard {
    env: Env,
    remaining: VecDeque<ObjectRef<false>>,
}

impl DescriptorGuard {
    /// Take ownership of an unbridged descriptor list and its environment.
    fn new(
        env: &Env,
        descriptors: Vec<ObjectRef<false>>,
    ) -> Self {
        Self { env: *env, remaining: descriptors.into() }
    }

    /// Take the next descriptor in caller order.
    fn pop(&mut self) -> Option<ObjectRef<false>> {
        self.remaining.pop_front()
    }
}

impl Drop for DescriptorGuard {
    fn drop(&mut self) {
        for descriptor in self.remaining.drain(..) {
            let _ = descriptor.unref(&self.env);
        }
    }
}

/// How converted descriptors update the options-payload view.
enum DescriptorViewUpdate {
    /// Replace the view with the entry descriptors.
    Seed,
    /// Append only the descriptors that are new to the view.
    Append,
}

/// Bridge raw descriptors into shared adapters and update the descriptor view.
///
/// A descriptor that fails mid-list releases every reference already on
/// `refs`, and the guard releases every descriptor that conversion had not
/// reached.
/// The view update happens only after the whole list converts, so a failure
/// leaves the view untouched. A descriptor whose internal id already has a
/// bridged adapter reuses that adapter instead of bridging again.
fn convert_plugins(
    env: &Env,
    descriptors: Vec<ObjectRef<false>>,
    refs: &Arc<RefList>,
    update: DescriptorViewUpdate,
) -> Result<Vec<SharedPluginable>> {
    let mut adapters: Vec<SharedPluginable> =
        Vec::with_capacity(descriptors.len());

    let mut bag: Vec<BagDescriptor> = Vec::with_capacity(descriptors.len());

    let mut guard: DescriptorGuard = DescriptorGuard::new(env, descriptors);

    while let Some(descriptor) = guard.pop() {
        // Root the reference before materializing. A materialize failure must
        // still leave the already-rooted descriptor releaseable.
        let shared: SharedRef = SharedRef::from_ref(descriptor);

        refs.push_ref(shared.clone());

        let object: napi::bindgen_prelude::Object<'static> =
            match shared.get(env) {
                | Ok(object) => object,
                | Err(error) => {
                    let _ = refs.release(env);

                    return Err(error);
                },
            };

        let id: Option<u32> = match object.get::<u32>(PLUGIN_ID) {
            | Ok(id) => id,
            | Err(error) => {
                let _ = refs.release(env);

                return Err(error);
            },
        };

        bag.push(BagDescriptor { id, reference: shared.clone() });

        if let Some(id) = id
            && let Some(existing) = refs.bridged_plugin(id)
        {
            adapters.push(existing);
            continue;
        }

        match bridge_plugin(&object, refs, &shared) {
            | Ok(plugin) => {
                if let Some(id) = id {
                    refs.remember_bridged(id, Arc::clone(&plugin));
                }

                adapters.push(plugin);
            },
            | Err(error) => {
                let _ = refs.release(env);

                return Err(error);
            },
        }
    }

    match update {
        | DescriptorViewUpdate::Seed => refs.seed_bag(bag),
        | DescriptorViewUpdate::Append => refs.append_bag(bag),
    }

    Ok(adapters)
}

/// Bridge every entry descriptor into a shared adapter.
///
/// The entry descriptors seed the options-payload descriptor view in
/// registration order. The seed keeps duplicate entries of one instance.
pub fn to_plugins(
    env: &Env,
    descriptors: Vec<ObjectRef<false>>,
    refs: &Arc<RefList>,
) -> Result<Vec<SharedPluginable>> {
    convert_plugins(env, descriptors, refs, DescriptorViewUpdate::Seed)
}

/// Bridge the descriptors an `options` hook added into shared adapters.
///
/// The descriptors append to the options-payload view, so later hooks observe
/// the full settled plugin list. A descriptor whose id is already in the view
/// is skipped. An omitted `plugins` field yields an empty list and adds
/// nothing.
pub fn to_patch_plugins(
    env: &Env,
    descriptors: Vec<ObjectRef<false>>,
    refs: &Arc<RefList>,
) -> Result<Vec<SharedPluginable>> {
    convert_plugins(env, descriptors, refs, DescriptorViewUpdate::Append)
}

#[cfg(test)]
mod tests {
    use super::*;

    // A failure-path integration test would need a non-empty descriptor list,
    // and creating or releasing a descriptor needs a live JS runtime. These
    // unit tests have no runtime, so only the empty-list paths are tested here.

    // The empty-descriptor path never dereferences the env. `Env::from_raw`
    // keeps the test free of a JS runtime.
    fn env() -> Env {
        napi::Env::from_raw(std::ptr::null_mut())
    }

    #[test]
    fn test_to_plugins_empty_seeds_empty_bag() {
        let env: Env = env();
        let refs: Arc<RefList> = Arc::new(RefList::new());

        let plugins: Vec<SharedPluginable> =
            to_plugins(&env, Vec::new(), &refs).expect("empty seed bridges");

        assert!(plugins.is_empty());
        assert_eq!(refs.bag_snapshot().len(), 0);
        assert!(refs.is_empty());
    }

    #[test]
    fn test_to_patch_plugins_empty_keeps_bag() {
        let env: Env = env();
        let refs: Arc<RefList> = Arc::new(RefList::new());

        refs.seed_bag(vec![
            BagDescriptor::stub(Some(1)),
            BagDescriptor::stub(Some(2)),
        ]);

        assert_eq!(refs.bag_snapshot().len(), 2);

        let plugins: Vec<SharedPluginable> =
            to_patch_plugins(&env, Vec::new(), &refs)
                .expect("empty patch bridges");

        assert!(plugins.is_empty());
        assert_eq!(refs.bag_snapshot().len(), 2);
        assert!(refs.is_empty());
    }
}
