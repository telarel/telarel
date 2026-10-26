use std::sync::Arc;

use napi::Env;
use oxc::span::SourceType as OxcSourceType;

use telarel_common::{
    Ast, CodeResult, GenerateOptions, Language, ResolvableOptions,
    ResolvedOptions, resolve_options,
};
use telarel_plugin::SharedPluginable;

use crate::_types::ast::JsAstInput;
use crate::_types::options::JsGenerateOptions;
use crate::_types::results::JsCodeResult;
use crate::ast::{
    effective_language, js_to_ast, parse_ast_language, source_type_for_generate,
};
use crate::plugin::build::to_plugins;
use crate::plugin::hooks::RefList;
use crate::tasks::function::{FunctionTask, Run};
use crate::tasks::{anyhow_to_napi, run_core};

/// Generate code from an AST with plugins, in memory.
#[napi_derive::napi(ts_return_type = "Promise<JsCodeResult>")]
pub fn generate(
    env: Env,
    ast: JsAstInput,
    options: JsGenerateOptions,
) -> napi::Result<napi::bindgen_prelude::AsyncTask<FunctionTask<JsCodeResult>>>
{
    let options_rust: GenerateOptions = options.to_generate_options()?;

    // The precedence is explicit options, then the input AST's grammar, then
    // the grammar inferred from the resolved file. The module kind stays
    // absent so the generated program's own `sourceType` settles it.
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

    let source_type: OxcSourceType = source_type_for_generate(language);

    let refs: Arc<RefList> = Arc::new(RefList::new());

    let plugins: Vec<SharedPluginable> =
        to_plugins(&env, options.plugins, &refs)?;

    let run: Run<JsCodeResult> = Box::new(move || {
        let ast: Ast = js_to_ast(input, source_type).map_err(anyhow_to_napi)?;

        let result: CodeResult = run_core(async move {
            telarel_core::generate(ast, options_rust, plugins).await
        })?;

        Ok(JsCodeResult::from(result))
    });

    Ok(napi::bindgen_prelude::AsyncTask::new(FunctionTask::new(
        run,
        Some(refs),
    )))
}
