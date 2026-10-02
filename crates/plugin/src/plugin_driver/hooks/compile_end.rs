use crate::_types::context::PluginContext;
use crate::_types::hooks::compile_end::CompileEndArgs;
use crate::plugin::pluginable::SharedPluginable;

/// Run the `compile_end` hook on every plugin in registration order.
/// Notify-only: results are ignored. Every plugin's hook runs even after one fails;
/// the FIRST error (context-wrapped) is returned only after the loop.
pub async fn compile_end(
    plugins: &[SharedPluginable],
    ctx: &PluginContext<'_>,
    args: &CompileEndArgs,
) -> anyhow::Result<()> {
    let mut first: Option<anyhow::Error> = None;

    for plugin in plugins {
        if let Err(error) = plugin.call_compile_end(ctx, args).await {
            let error: anyhow::Error =
                error.context(format!("`{}` compile_end", plugin.call_name()));

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
