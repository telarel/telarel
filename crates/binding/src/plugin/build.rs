use std::sync::Arc;

use napi::bindgen_prelude::ObjectRef;
use napi::{Env, Result};

use telarel_plugin::SharedPluginable;

use crate::plugin::builtin::to_builtin_plugin;
use crate::plugin::hooks::{RefList, SharedRef};
use crate::plugin::pluginable::JsPlugin;

/// The error raised when a JS plugin object carries no `name`.
pub const NAME_REQUIRED: &str = "plugin `name` is required";

/// Internal descriptor key shared with the JS wrapper; must match
/// `PLUGIN_ID` in `packages/telarel/src/bridges/plugin.ts`.
const PLUGIN_ID: &str = "__telarelPluginId";

/// Bridge one raw JS plugin descriptor into a shared adapter; the caller has
/// already rooted its reference. Builtin markers dispatch to Rust before any
/// hook scan (entry and mid-fixpoint alike); plain JS plugins become
/// [`JsPlugin`].
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
/// The returned bag REPLACES the current descriptor view via
/// [`RefList::set_bag`]; a failure mid-list releases every reference already
/// on `refs` and leaves the view untouched. A descriptor whose internal id
/// already has a bridged adapter reuses that adapter instead of bridging
/// again.
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

        let id: Option<u32> = match object.get::<u32>(PLUGIN_ID) {
            | Ok(id) => id,
            | Err(error) => {
                let _ = refs.release(env);

                return Err(error);
            },
        };

        if let Some(id) = id
            && let Some(existing) = refs.bridged_plugin(id)
        {
            adapters.push(existing);
            continue;
        }

        match bridge_plugin(&object, refs) {
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
