use std::sync::Arc;

use napi::Env;

use telarel_common::{
    Ast, AstResult, Language, ResolvableOptions, ResolvedOptions,
    RewriteOptions, resolve_options,
};
use telarel_plugin::SharedPluginable;

use crate::_types::ast::{JsAstInput, JsAstResult};
use crate::_types::options::JsRewriteOptions;
use crate::ast::{
    effective_language, js_to_ast, parse_ast_language,
    source_type_for_ast_input,
};
use crate::plugin::build::to_plugins;
use crate::plugin::hooks::RefList;
use crate::tasks::function::{FunctionTask, Run};
use crate::tasks::{anyhow_to_napi, run_core};

/// Rewrite an AST with plugins, in memory.
#[napi_derive::napi(ts_return_type = "Promise<JsAstResult>")]
pub fn rewrite(
    env: Env,
    ast: JsAstInput,
    options: JsRewriteOptions,
) -> napi::Result<napi::bindgen_prelude::AsyncTask<FunctionTask<JsAstResult>>> {
    let options_rust: RewriteOptions = options.to_rewrite_options()?;

    // The grammar comes from explicit options, the input AST, or the file extension,
    // in that order. The module kind comes from an explicit `sourceType` other than
    // `"unambiguous"`, the program's `sourceType`, or the file extension, in that order.
    let resolved: ResolvedOptions = resolve_options(
        ResolvableOptions::from(options_rust.clone()),
        Vec::new(),
    );

    let input: crate::ast::JsAstInput = ast.into();

    let language: Language = effective_language(
        options_rust.language,
        parse_ast_language(input.language.as_deref())?,
        resolved.language,
    );

    let source_type = source_type_for_ast_input(
        &resolved.file,
        language,
        options_rust.source_type,
        &input.program,
    );

    let refs: Arc<RefList> = Arc::new(RefList::new());

    let plugins: Vec<SharedPluginable> =
        to_plugins(&env, options.plugins, &refs)?;

    let run: Run<JsAstResult> = Box::new(move || {
        let ast: Ast = js_to_ast(input, source_type).map_err(anyhow_to_napi)?;

        let result: AstResult = run_core(async move {
            telarel_core::rewrite(ast, options_rust, plugins).await
        })?;

        JsAstResult::from_ast(&result.ast).map_err(anyhow_to_napi)
    });

    Ok(napi::bindgen_prelude::AsyncTask::new(FunctionTask::new(
        run,
        Some(refs),
    )))
}
