use std::sync::Arc;

use napi::Env;

use telarel_common::{CodeResult, CompileOptions};
use telarel_plugin::SharedPluginable;

use crate::_types::options::JsCompileOptions;
use crate::_types::results::JsCodeResult;
use crate::plugin::build::to_plugins;
use crate::plugin::hooks::RefList;
use crate::tasks::function::{FunctionTask, Run};
use crate::tasks::run_core;

/// Compile a source text with plugins, in memory.
#[napi_derive::napi(ts_return_type = "Promise<JsCodeResult>")]
pub fn compile(
    env: Env,
    code: String,
    options: JsCompileOptions,
) -> napi::Result<napi::bindgen_prelude::AsyncTask<FunctionTask<JsCodeResult>>>
{
    let options_rust: CompileOptions = options.to_compile_options()?;

    let refs: Arc<RefList> = Arc::new(RefList::new());

    let plugins: Vec<SharedPluginable> =
        to_plugins(&env, options.plugins, &refs)?;

    let run: Run<JsCodeResult> = Box::new(move || {
        let result: CodeResult = run_core(async move {
            telarel_core::compile(code, options_rust, plugins).await
        })?;

        Ok(JsCodeResult::from(result))
    });

    Ok(napi::bindgen_prelude::AsyncTask::new(FunctionTask::new(
        run,
        Some(refs),
    )))
}
