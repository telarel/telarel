use anyhow::Context;

use crate::_types::context::PluginContext;
use crate::_types::hooks::compile_start::CompileStartArgs;
use crate::plugin::pluginable::SharedPluginable;

/// Run the `compile_start` hook on every plugin in registration order;
/// the first error aborts. Notify-only: results are ignored.
pub async fn compile_start(
    plugins: &[SharedPluginable],
    ctx: &PluginContext<'_>,
    args: &CompileStartArgs,
) -> anyhow::Result<()> {
    for plugin in plugins {
        plugin.call_compile_start(ctx, args).await.with_context(|| {
            format!("`{}` compile_start", plugin.call_name())
        })?;
    }

    Ok(())
}
