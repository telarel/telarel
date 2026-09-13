use anyhow::Context;
use telarel_common::CompileContext;

use crate::_types::hooks::post::PostArgs;
use crate::plugin::pluginable::SharedPluginable;

/// Run the `post` hook on every plugin in registration order; the first error aborts.
pub async fn post(
    plugins: &[SharedPluginable],
    ctx: &CompileContext<'_>,
    args: &PostArgs<'_>,
) -> anyhow::Result<()> {
    for plugin in plugins {
        plugin
            .call_post(ctx, args)
            .await
            .with_context(|| format!("`{}` post", plugin.call_name()))?;
    }

    Ok(())
}
