use telarel_common::{
    AstResult, Parser, ParserOptions, ResolvedOptions, resolve_options,
};

use crate::_types::ast::JsAstResult;
use crate::_types::options::JsParserOptions;
use crate::tasks::anyhow_to_napi;
use crate::tasks::error_to_napi;
use crate::tasks::function::{FunctionTask, Run};

/// Parse a source text with the builtin parser, in memory.
///
/// Backs the builtin `createParser`; no plugin chain runs.
#[napi_derive::napi(ts_return_type = "Promise<JsAstResult>")]
pub fn builtin_parse(
    code: String,
    options: JsParserOptions,
) -> napi::Result<napi::bindgen_prelude::AsyncTask<FunctionTask<JsAstResult>>> {
    // Resolve the defaults (cwd, file, grammar, module kind, source map) the
    // same way the pipeline's builtin parser fallback does.
    let resolved: ResolvedOptions =
        resolve_options(options.to_resolvable()?, Vec::new());

    let options_rust: ParserOptions = ParserOptions::from(&resolved);

    let run: Run<JsAstResult> = Box::new(move || {
        let result: AstResult =
            Parser::new(code, options_rust).parse().map_err(error_to_napi)?;

        JsAstResult::from_ast(&result.ast).map_err(anyhow_to_napi)
    });

    Ok(napi::bindgen_prelude::AsyncTask::new(FunctionTask::new(run, None)))
}
