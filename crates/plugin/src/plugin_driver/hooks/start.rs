use crate::_types::context::PluginContext;
use crate::_types::hooks::start::StartArgs;
use crate::PluginError;
use crate::plugin::pluginable::SharedPluginable;

/// Run the `start` hook on every settled plugin in meta-ranked order. The
/// first error aborts. Notify-only: results are ignored.
pub async fn start(
    plugins: &[SharedPluginable],
    ctx: &PluginContext<'_>,
    args: &StartArgs,
) -> Result<(), PluginError> {
    for plugin in plugins {
        plugin.call_start(ctx, args).await?;
    }

    Ok(())
}
