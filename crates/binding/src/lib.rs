mod _types;
mod plugin;
mod tasks;

use std::sync::Arc;

use napi::Env;
use napi::bindgen_prelude::{Array, JsValue, Object, ObjectRef};

use telarel_common::CompileOptions;
use telarel_plugin::SharedPluginable;

use crate::_types::options::{JsOptions, to_compile_options};
use crate::plugin::build::bridge_plugin;
use crate::plugin::hooks::{RefList, SharedRef};
use crate::tasks::compile::CompileTask;

/// Compile a file with plugins, in memory.
#[napi_derive::napi(ts_return_type = "Promise<JsCompileResult>")]
pub fn compile(
    env: Env,
    options: JsOptions,
) -> napi::Result<napi::bindgen_prelude::AsyncTask<CompileTask>> {
    // Convert the options before rooting anything: a validation failure
    // (for example, an invalid `sourceType`) must not leak rooted references.
    let options_rust: CompileOptions = to_compile_options(&options)?;

    // The dynamic release list of this compile: seeded with the plugin array
    // and every ENTRY descriptor here, extended by the `options` hook as the
    // fixpoint returns bags with late plugins, drained in `Task::finally`.
    let refs: Arc<RefList> = Arc::new(RefList::new());

    let plugin_array: Array<'static> = Array::from_vec(
        &env,
        options.plugins.iter().collect::<Vec<&ObjectRef<false>>>(),
    )?;

    let plugin_array: Object<'static> =
        Object::from_raw(env.raw(), JsValue::raw(&plugin_array));

    refs.push_object(&plugin_array)?;

    let mut plugins: Vec<SharedPluginable> =
        Vec::with_capacity(options.plugins.len());

    // The ENTRY bag's descriptor view: the user's original plugin list, in
    // order. Seeded below with clones of the same refs pushed onto the
    // release list, so the FIRST `options` call's payload is exact.
    let mut entry_descriptors: Vec<SharedRef> = Vec::new();

    for descriptor in options.plugins.into_iter() {
        // The descriptor is ALREADY rooted (the `JsOptions` conversion
        // created the reference); wrap the SAME root onto the release list
        // so a bridging failure below releases it too.
        let object: Object<'static> =
            crate::plugin::hooks::materialize(&descriptor, &env)?;

        let shared: SharedRef = SharedRef::from_ref(descriptor);

        refs.push_ref(shared.clone());

        entry_descriptors.push(shared);

        match bridge_plugin(&object, &refs) {
            | Ok(plugin) => plugins.push(plugin),
            | Err(error) => {
                // Bridging failed before the async task exists, so nothing
                // will run `Task::finally`. `compile` runs on the JS thread
                // (the only thread `SharedRef::release` may touch), so release
                // every rooted reference here before the error propagates;
                // `release` is idempotent, so double-release is safe.
                let _ = refs.release(&env);

                return Err(error);
            },
        }
    }

    // Every entry descriptor bridged successfully: the entry bag is the
    // first CURRENT bag the `options` fixpoint folds.
    refs.set_bag(entry_descriptors);

    Ok(napi::bindgen_prelude::AsyncTask::new(CompileTask::new(
        options_rust,
        plugins,
        refs,
    )))
}
