use telarel_common::{AstResult, CompileError, Parser, ParserOptions};

use crate::_types::context::PluginContext;
use crate::_types::hooks::parse::ParseArgs;
use crate::PluginError;
use crate::plugin::pluginable::SharedPluginable;

/// Run the `parse` hook over every plugin in meta-ranked order.
///
/// First-win: the first `Ok(Some)` result wins and the remaining hooks are
/// skipped. `Ok(None)` defers to the next plugin. The first error aborts and
/// is propagated unchanged. When every hook defers, the builtin parser runs
/// and its error is returned as-is.
pub async fn parse(
    plugins: &[SharedPluginable],
    ctx: &PluginContext<'_>,
    code: &str,
    options: &ParserOptions,
    builtin: &Parser,
) -> Result<AstResult, CompileError> {
    for plugin in plugins {
        let args: ParseArgs<'_> = ParseArgs { code, options };

        let result: Option<AstResult> = plugin
            .call_parse(ctx, args)
            .await
            .map_err(PluginError::into_compile_error)?;

        if let Some(result) = result {
            return Ok(result);
        }
    }

    builtin.clone().parse()
}
