use anyhow::Context;

use crate::_types::context::PluginContext;
use crate::_types::hooks::finalize::FinalizeArgs;
use crate::plugin::pluginable::SharedPluginable;

/// Run the `finalize` hook on every plugin in meta-ranked order as a
/// read-only observer of the final AST; the first error aborts. Return
/// values are ignored.
pub async fn finalize(
    plugins: &[SharedPluginable],
    ctx: &PluginContext<'_>,
    args: FinalizeArgs<'_>,
) -> anyhow::Result<()> {
    for plugin in plugins {
        plugin
            .call_finalize(ctx, FinalizeArgs { ast: args.ast })
            .await
            .with_context(|| format!("`{}` finalize", plugin.call_name()))?;
    }

    Ok(())
}
