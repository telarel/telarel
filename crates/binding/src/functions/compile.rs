use std::sync::Arc;

use napi::Env;

use telarel_common::CompileOptions;
use telarel_plugin::SharedPluginable;

use crate::_types::options::JsOptions;
use crate::_types::options::to_compile_options;
use crate::plugin::build::to_plugins;
use crate::plugin::hooks::RefList;
use crate::tasks::compile::CompileTask;

/// Compile a file with plugins, in memory.
///
/// TEMPORARY ADAPTER (Task-4 deletion target): keeps the old
/// `compile(options)` JS shape; the source text is lifted out of `options` and
/// travels beside the code-free `CompileOptions`.
#[napi_derive::napi(ts_return_type = "Promise<JsCompileResult>")]
pub fn compile(
    env: Env,
    options: JsOptions,
) -> napi::Result<napi::bindgen_prelude::AsyncTask<CompileTask>> {
    // The adapter's copy of the source text: `core::compile` takes it as its
    // first argument, so it no longer rides inside `CompileOptions`.
    let code: String = options.code.clone();

    // Convert the options before rooting anything: a validation failure
    // (for example, an invalid `sourceType` or `sourceMap`) must not leak
    // rooted references.
    let options_rust: CompileOptions = to_compile_options(&options)?;

    // The dynamic release list of this compile: seeded with every ENTRY
    // descriptor, extended by the `options` hook as the fixpoint returns bags
    // with late plugins, drained in `Task::finally`.
    let refs: Arc<RefList> = Arc::new(RefList::new());

    // Bridge the entry bag, rooting every descriptor onto `refs` and seeding
    // the first CURRENT bag the `options` fixpoint folds. `to_plugins`
    // releases the rooted references on failure (no task exists yet to run
    // `Task::finally`).
    let plugins: Vec<SharedPluginable> =
        to_plugins(&env, options.plugins, &refs)?;

    Ok(napi::bindgen_prelude::AsyncTask::new(CompileTask::new(
        code,
        options_rust,
        plugins,
        refs,
    )))
}
