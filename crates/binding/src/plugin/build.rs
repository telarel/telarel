use std::sync::Arc;

use napi::{Env, Result};

use telarel_plugin::SharedPluginable;

use crate::plugin::builtin::to_builtin_plugin;
use crate::plugin::hooks::SharedRef;
use crate::plugin::pluginable::JsPlugin;

/// The error raised when a JS plugin object carries no `name`.
pub const NAME_REQUIRED: &str = "plugin `name` is required";

/// Bridge every raw JS plugin object into a shared adapter.
///
/// `plugin_array` is the rooted JS array of all plugin objects, embedded
/// into `options` hook payloads. Every reference is rooted and released by
/// the `compile` task when the compile finishes.
///
/// A builtin marker (`{ __builtin: true, name, options }`) is dispatched to
/// its Rust plugin before the hook scan; plain JS plugins keep the existing
/// `JsPlugin` path.
pub fn to_plugins(
    env: &Env,
    plugins: &[SharedRef],
    plugin_array: &SharedRef,
) -> Result<Vec<SharedPluginable>> {
    let mut adapters: Vec<SharedPluginable> = Vec::with_capacity(plugins.len());

    for plugin in plugins {
        let object = plugin.get(env)?;

        // Check if `__builtin` equal to `true` for builtin plugin
        let is_builtin: bool = object
            .get::<serde_json::Value>("__builtin")?
            .and_then(|value: serde_json::Value| value.as_bool())
            == Some(true);

        if is_builtin {
            let name: String =
                object.get::<String>("name")?.ok_or_else(|| {
                    napi::Error::new(
                        napi::Status::InvalidArg,
                        NAME_REQUIRED.to_owned(),
                    )
                })?;

            let options: Option<serde_json::Value> =
                object.get::<serde_json::Value>("options")?;

            let adapter: SharedPluginable = to_builtin_plugin(&name, options)?;

            adapters.push(adapter);

            continue;
        }

        let adapter: JsPlugin = JsPlugin::from_object(&object, plugin_array)?;

        adapters.push(Arc::new(adapter));
    }

    Ok(adapters)
}
