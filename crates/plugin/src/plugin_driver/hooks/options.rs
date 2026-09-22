use anyhow::Context;

use telarel_common::CompileOptions;

use crate::plugin::pluginable::SharedPluginable;

/// Run the `options` hook chain.
///
/// Each plugin mutates the same options in place;
/// mutations are visible to the following plugins.
pub async fn options(
    plugins: &[SharedPluginable],
    options: &mut CompileOptions,
) -> anyhow::Result<()> {
    for plugin in plugins {
        plugin
            .call_options(&mut *options)
            .await
            .with_context(|| format!("`{}` options", plugin.call_name()))?;
    }

    Ok(())
}
