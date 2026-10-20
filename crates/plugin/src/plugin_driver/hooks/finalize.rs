use crate::_types::context::PluginContext;
use crate::_types::hooks::finalize::FinalizeArgs;
use crate::PluginError;
use crate::plugin::pluginable::SharedPluginable;

/// Run the `finalize` hook on every plugin in meta-ranked order as a
/// read-only observer of the final AST; the first error aborts. Return
/// values are ignored.
pub async fn finalize(
    plugins: &[SharedPluginable],
    ctx: &PluginContext<'_>,
    args: FinalizeArgs<'_>,
) -> Result<(), PluginError> {
    for plugin in plugins {
        plugin.call_finalize(ctx, FinalizeArgs { ast: args.ast }).await?;
    }

    Ok(())
}
