use std::sync::Arc;

use napi::{Env, Result};

use telarel_plugin::SharedPluginable;

use crate::plugin::hooks::SharedRef;
use crate::plugin::pluginable::JsPlugin;

/// Bridge every raw JS plugin object into a shared adapter.
///
/// `plugin_array` is the rooted JS array of all plugin objects, embedded
/// into `options` hook payloads. Every reference is rooted and released by
/// the `compile` task when the compile finishes.
pub fn to_plugins(
    env: &Env,
    plugins: &[SharedRef],
    plugin_array: &SharedRef,
) -> Result<Vec<SharedPluginable>> {
    let mut adapters: Vec<SharedPluginable> = Vec::with_capacity(plugins.len());

    for plugin in plugins {
        let object = plugin.get(env)?;

        let adapter: JsPlugin = JsPlugin::from_object(&object, plugin_array)?;

        adapters.push(Arc::new(adapter));
    }

    Ok(adapters)
}
