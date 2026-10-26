use oxc::span::SourceType as OxcSourceType;

use telarel_common::{
    Ast, CodeResult, Generator, GeneratorOptions, Language, ResolvedOptions,
    resolve_options,
};

use crate::_types::ast::JsAstInput;
use crate::_types::options::JsGeneratorOptions;
use crate::_types::results::JsCodeResult;
use crate::ast::{
    effective_language, js_to_ast, parse_ast_language, source_type_for_generate,
};
use crate::tasks::anyhow_to_napi;
use crate::tasks::error_to_napi;
use crate::tasks::function::{FunctionTask, Run};

/// Generate code from an AST with the builtin generator, in memory.
///
/// Backs the builtin `createGenerator`. No plugin chain runs.
#[napi_derive::napi(ts_return_type = "Promise<JsCodeResult>")]
pub fn builtin_generate(
    ast: JsAstInput,
    options: JsGeneratorOptions,
) -> napi::Result<napi::bindgen_prelude::AsyncTask<FunctionTask<JsCodeResult>>>
{
    // The generator options carry no grammar. The input AST's language wins,
    // then the resolved file. The program JSON owns the module kind.
    let resolved: ResolvedOptions =
        resolve_options(options.to_resolvable()?, Vec::new());

    let options_rust: GeneratorOptions = GeneratorOptions::from(&resolved);

    let input: crate::ast::JsAstInput = ast.into();

    let language: Language = effective_language(
        None,
        parse_ast_language(input.language.as_deref())?,
        resolved.language,
    );

    let source_type: OxcSourceType = source_type_for_generate(language);

    let run: Run<JsCodeResult> = Box::new(move || {
        let ast: Ast = js_to_ast(input, source_type).map_err(anyhow_to_napi)?;

        let result: CodeResult = Generator::new(&ast, options_rust)
            .generate()
            .map_err(error_to_napi)?;

        Ok(JsCodeResult::from(result))
    });

    Ok(napi::bindgen_prelude::AsyncTask::new(FunctionTask::new(run, None)))
}
