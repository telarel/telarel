use telarel_common::{
    Ast, CodeResult, CompileError, Generator, GeneratorOptions,
};

use crate::_types::context::PluginContext;
use crate::_types::hooks::generate::GenerateArgs;
use crate::PluginError;
use crate::plugin::pluginable::SharedPluginable;

/// Run the `generate` hook over every plugin in meta-ranked order.
///
/// First-win: the first `Ok(Some)` result wins and the remaining hooks are
/// skipped. `Ok(None)` defers to the next plugin. The first error aborts and
/// is propagated unchanged. When every hook defers, the builtin generator
/// runs and its error is returned as-is.
pub async fn generate(
    plugins: &[SharedPluginable],
    ctx: &PluginContext<'_>,
    ast: &Ast,
    options: &GeneratorOptions,
    builtin: &Generator<'_>,
) -> Result<CodeResult, CompileError> {
    for plugin in plugins {
        let args: GenerateArgs<'_> = GenerateArgs { ast, options };

        let result: Option<CodeResult> = plugin
            .call_generate(ctx, args)
            .await
            .map_err(PluginError::into_compile_error)?;

        if let Some(result) = result {
            return Ok(result);
        }
    }

    builtin.clone().generate()
}
