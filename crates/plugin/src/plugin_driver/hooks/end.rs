use crate::_types::context::PluginContext;
use crate::_types::hooks::end::EndArgs;
use crate::plugin::pluginable::SharedPluginable;

/// Run the `end` hook on every settled plugin in meta-ranked order.
/// Notify-only: results are ignored. Every plugin's hook runs even after one
/// fails; the FIRST error (context-wrapped) is returned only after the loop.
pub async fn end(
    plugins: &[SharedPluginable],
    ctx: &PluginContext<'_>,
    args: &EndArgs,
) -> anyhow::Result<()> {
    let mut first: Option<anyhow::Error> = None;

    for plugin in plugins {
        if let Err(error) = plugin.call_end(ctx, args).await {
            let error: anyhow::Error =
                error.context(format!("`{}` end", plugin.call_name()));

            if first.is_none() {
                first = Some(error);
            }
        }
    }

    match first {
        | Some(error) => Err(error),
        | None => Ok(()),
    }
}
