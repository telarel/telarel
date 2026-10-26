use crate::_types::context::PluginContext;
use crate::_types::hooks::end::EndArgs;
use crate::PluginError;
use crate::plugin::pluginable::SharedPluginable;

/// Run the `end` hook on every settled plugin in meta-ranked order.
/// Notify-only: results are ignored. Every plugin's hook runs even after one
/// fails. The FIRST error is returned only after the loop.
pub async fn end(
    plugins: &[SharedPluginable],
    ctx: &PluginContext<'_>,
    args: &EndArgs,
) -> Result<(), PluginError> {
    let mut first: Option<PluginError> = None;

    for plugin in plugins {
        if let Err(error) = plugin.call_end(ctx, args).await {
            first = first.or(Some(error));
        }
    }

    match first {
        | Some(error) => Err(error),
        | None => Ok(()),
    }
}
