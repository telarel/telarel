use crate::_types::context::PluginContext;
use crate::_types::hooks::prepare::PrepareArgs;
use crate::PluginError;
use crate::plugin::pluginable::SharedPluginable;

/// Run the `prepare` hook on every plugin in meta-ranked order as a
/// read-only observer of the parsed AST; the first error aborts. Return
/// values are ignored.
pub async fn prepare(
    plugins: &[SharedPluginable],
    ctx: &PluginContext<'_>,
    args: PrepareArgs<'_>,
) -> Result<(), PluginError> {
    for plugin in plugins {
        plugin.call_prepare(ctx, PrepareArgs { ast: args.ast }).await?;
    }

    Ok(())
}
