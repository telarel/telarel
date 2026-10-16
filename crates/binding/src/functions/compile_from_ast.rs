use std::sync::Arc;

use napi::Env;

use telarel_common::{
    Ast, CodeResult, CompileFromAstOptions, ResolvableOptions, ResolvedOptions,
    resolve_options,
};
use telarel_plugin::SharedPluginable;

use crate::_types::ast::JsAstInput;
use crate::_types::options::JsCompileFromAstOptions;
use crate::_types::results::JsCodeResult;
use crate::ast::{js_to_ast, source_type_for_options};
use crate::plugin::build::to_plugins;
use crate::plugin::hooks::RefList;
use crate::tasks::function::{FunctionTask, Run};
use crate::tasks::{anyhow_to_napi, run_core};

/// Compile an existing AST through rewrite and generation, with plugins.
#[napi_derive::napi(ts_return_type = "Promise<JsCodeResult>")]
pub fn compile_from_ast(
    env: Env,
    ast: JsAstInput,
    options: JsCompileFromAstOptions,
) -> napi::Result<napi::bindgen_prelude::AsyncTask<FunctionTask<JsCodeResult>>>
{
    let options_rust: CompileFromAstOptions =
        options.to_compile_from_ast_options()?;

    // The read-back grammar comes from the same resolved context the pipeline
    // applies, so `language`/`sourceType` default identically.
    let resolved: ResolvedOptions = resolve_options(
        ResolvableOptions::from(options_rust.clone()),
        Vec::new(),
    );

    let source_type = source_type_for_options(
        &resolved.file,
        Some(resolved.language),
        resolved.source_type,
    );

    let refs: Arc<RefList> = Arc::new(RefList::new());

    let plugins: Vec<SharedPluginable> =
        to_plugins(&env, options.plugins, &refs)?;

    let input: crate::ast::JsAstInput = ast.into();

    let run: Run<JsCodeResult> = Box::new(move || {
        let ast: Ast = js_to_ast(input, source_type).map_err(anyhow_to_napi)?;

        let result: CodeResult = run_core(async move {
            telarel_core::compile_from_ast(ast, options_rust, plugins).await
        })?;

        Ok(JsCodeResult::from(result))
    });

    Ok(napi::bindgen_prelude::AsyncTask::new(FunctionTask::new(
        run,
        Some(refs),
    )))
}
