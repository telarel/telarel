use std::sync::Arc;

use napi::Result;

use telarel_plugin::SharedPluginable;

use crate::plugin::builtin::to_builtin_plugin;
use crate::plugin::hooks::RefList;
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
