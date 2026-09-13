use anyhow::Context;
use telarel_common::CompileContext;

use crate::_types::hooks::pre::PreArgs;
use crate::plugin::pluginable::SharedPluginable;

/// Run the `pre` hook on every plugin in registration order; the first error aborts.
pub async fn pre(
    plugins: &[SharedPluginable],
    ctx: &CompileContext<'_>,
    args: &PreArgs<'_>,
) -> anyhow::Result<()> {
    for plugin in plugins {
        plugin
            .call_pre(ctx, args)
            .await
            .with_context(|| format!("`{}` pre", plugin.call_name()))?;
    }

    Ok(())
}
