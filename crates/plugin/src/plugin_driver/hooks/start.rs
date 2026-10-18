use anyhow::Context;

use crate::_types::context::PluginContext;
use crate::_types::hooks::start::StartArgs;
use crate::plugin::pluginable::SharedPluginable;

/// Run the `start` hook on every settled plugin in meta-ranked order;
/// the first error aborts. Notify-only: results are ignored.
pub async fn start(
    plugins: &[SharedPluginable],
    ctx: &PluginContext<'_>,
    args: &StartArgs,
) -> anyhow::Result<()> {
    for plugin in plugins {
        plugin
            .call_start(ctx, args)
            .await
            .with_context(|| format!("`{}` start", plugin.call_name()))?;
    }

    Ok(())
}
