use std::sync::Arc;

use napi::bindgen_prelude::ObjectRef;
use napi::{Env, Result};

use telarel_plugin::SharedPluginable;

use crate::plugin::builtin::to_builtin_plugin;
use crate::plugin::hooks::{RefList, SharedRef};
use crate::plugin::pluginable::JsPlugin;

/// The error raised when a JS plugin object carries no `name`.
pub const NAME_REQUIRED: &str = "plugin `name` is required";

/// Bridge ONE raw plugin object into a shared adapter; its rooted reference
/// must already be on `refs` (late-plugin descriptors are pushed by the
/// `options` hook before this call; the entry bridge seeds the list first).
///
/// A builtin marker (`{ __builtin: true, name, options }`) is dispatched to
/// its Rust builtin before any hook scan; a plain JS plugin object is
/// bridged as a [`JsPlugin`]. The SAME dispatch applies mid-fixpoint, so a
/// plugin injected by the `options` hook participates like any entry one.
///
/// The caller roots the descriptor's reference (the entry bridge seeds the
/// list first; the `options` hook pushes late descriptors as it wraps) and
/// the release list drains in `CompileTask::finally`.
pub fn bridge_plugin(
    object: &napi::bindgen_prelude::Object<'static>,
    refs: &Arc<RefList>,
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
        Arc::new(JsPlugin::from_object(object, refs)?);

    Ok(adapter)
}

/// Bridge every raw JS plugin descriptor into a shared adapter.
///
/// The whole bag is converted together: each descriptor's already-rooted
/// reference moves onto `refs` (the compile's release list) and the bridged
/// adapters are returned in bag order. The returned bag REPLACES the current
/// descriptor view via [`RefList::set_bag`], so the `options` hook sees
/// exactly this list.
///
/// The caller owns rooting the descriptors; this function owns their
/// lifecycle from that point on. A materialize or bridging failure mid-list
/// releases every reference already on `refs` (the compile aborts before any
/// task exists to drain them) and leaves the bag view untouched.
pub fn to_plugins(
    env: &Env,
    descriptors: Vec<ObjectRef<false>>,
    refs: &Arc<RefList>,
) -> Result<Vec<SharedPluginable>> {
    let mut adapters: Vec<SharedPluginable> =
        Vec::with_capacity(descriptors.len());

    let mut bag: Vec<SharedRef> = Vec::with_capacity(descriptors.len());

    for descriptor in descriptors {
        // Root the reference BEFORE materializing: a materialize failure
        // must still leave the already-rooted descriptor releaseable.
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

        bag.push(shared);

        match bridge_plugin(&object, refs) {
            | Ok(plugin) => adapters.push(plugin),
            | Err(error) => {
                let _ = refs.release(env);

                return Err(error);
            },
        }
    }

    refs.set_bag(bag);

    Ok(adapters)
}

#[cfg(test)]
mod tests {
    use super::*;

    // The empty-descriptor path never dereferences the env; `Env::from_raw`
    // keeps the test free of a JS runtime.
    fn env() -> Env {
        napi::Env::from_raw(std::ptr::null_mut())
    }

    #[test]
    fn test_to_plugins_empty_replaces_bag() {
        let env: Env = env();
        let refs: Arc<RefList> = Arc::new(RefList::new());

        refs.set_bag(vec![SharedRef::stub(), SharedRef::stub()]);

        assert_eq!(refs.bag_snapshot().len(), 2);

        let plugins: Vec<SharedPluginable> =
            to_plugins(&env, Vec::new(), &refs).expect("empty bag bridges");

        assert!(plugins.is_empty());
        assert_eq!(refs.bag_snapshot().len(), 0);
        assert!(refs.is_empty());
    }
}
