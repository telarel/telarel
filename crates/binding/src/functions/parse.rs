use std::sync::Arc;

use napi::Env;

use telarel_common::{AstResult, ParseOptions};
use telarel_plugin::SharedPluginable;

use crate::_types::ast::JsAstResult;
use crate::_types::options::JsParseOptions;
use crate::plugin::build::to_plugins;
use crate::plugin::hooks::RefList;
use crate::tasks::function::{FunctionTask, Run};
use crate::tasks::{anyhow_to_napi, run_core};

/// Parse a source text with plugins, in memory.
#[napi_derive::napi(ts_return_type = "Promise<JsAstResult>")]
pub fn parse(
    env: Env,
    code: String,
    options: JsParseOptions,
) -> napi::Result<napi::bindgen_prelude::AsyncTask<FunctionTask<JsAstResult>>> {
    // Convert the options before rooting anything so a validation failure
    // cannot leak rooted references.
    let options_rust: ParseOptions = options.to_parse_options()?;

    // The dynamic release list for this call. It starts with every entry
    // descriptor, the `options` hook extends it, and the task's `finally`
    // drains it.
    let refs: Arc<RefList> = Arc::new(RefList::new());

    let plugins: Vec<SharedPluginable> =
        to_plugins(&env, options.plugins, &refs)?;

    let run: Run<JsAstResult> = Box::new(move || {
        let result: AstResult = run_core(async move {
            telarel_core::parse(code, options_rust, plugins).await
        })?;

        JsAstResult::from_ast(&result.ast).map_err(anyhow_to_napi)
    });

    Ok(napi::bindgen_prelude::AsyncTask::new(FunctionTask::new(
        run,
        Some(refs),
    )))
}
