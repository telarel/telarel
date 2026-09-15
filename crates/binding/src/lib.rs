mod _types;
mod plugin;
mod tasks;

use napi::Env;
use napi::bindgen_prelude::{Array, JsValue, Object, ObjectRef};

use telarel_common::CompileOptions;
use telarel_plugin::SharedPluginable;

use crate::_types::options::{JsOptions, to_compile_options};
use crate::plugin::build::to_plugins;
use crate::plugin::hooks::SharedRef;
use crate::tasks::compile::CompileTask;

/// Compile a file with plugins, in memory.
#[napi_derive::napi(ts_return_type = "Promise<JsCompileResult>")]
pub fn compile(
    env: Env,
    options: JsOptions,
) -> napi::Result<napi::bindgen_prelude::AsyncTask<CompileTask>> {
    // The metadata object is created ONCE per compile and shared by every
    // plugin through every hook payload. It is rooted so it survives for the
    // whole compilation; the reference is released when the compile finishes.
    let metadata: Object<'static> = Object::new(&env)?;

    let metadata: SharedRef = SharedRef::new(&metadata)?;

    // Root the plugin objects in one JS array; it is embedded into `options`
    // hook payloads.
    let plugin_array: Array<'_> = Array::from_vec(
        &env,
        options.plugins.iter().collect::<Vec<&ObjectRef<false>>>(),
    )?;

    let plugin_array: Object<'static> =
        Object::from_raw(env.raw(), JsValue::raw(&plugin_array));

    let plugin_array: SharedRef = SharedRef::new(&plugin_array)?;

    // Wrap the per-plugin references created during parsing so they can be
    // released with the rest of the compile's rooted references.
    let options_rust: CompileOptions = to_compile_options(&options);

    let plugin_refs: Vec<SharedRef> =
        options.plugins.into_iter().map(SharedRef::from_ref).collect();

    let plugins: Vec<SharedPluginable> =
        match to_plugins(&env, &plugin_refs, &metadata, &plugin_array) {
            | Ok(plugins) => plugins,
            | Err(error) => {
                // Bridging failed before the async task exists, so nothing
                // will run `Task::finally`. `compile` still runs on the JS
                // thread — the only thread `SharedRef::release` may touch —
                // so every rooted reference is released here before the
                // error propagates. Individual release failures are
                // secondary to the bridging error and cannot be reported
                // from this cleanup path; `release` is idempotent, so this
                // is safe even if some refs were already released.
                let _ = metadata.release(&env);

                let _ = plugin_array.release(&env);

                for reference in &plugin_refs {
                    let _ = reference.release(&env);
                }

                return Err(error);
            },
        };

    // Every rooted reference of this compile, released in `Task::finally`.
    let mut refs: Vec<SharedRef> = Vec::with_capacity(plugin_refs.len() + 2);

    refs.push(metadata);

    refs.push(plugin_array);

    refs.extend(plugin_refs);

    Ok(napi::bindgen_prelude::AsyncTask::new(CompileTask::new(
        options_rust,
        plugins,
        refs,
    )))
}
