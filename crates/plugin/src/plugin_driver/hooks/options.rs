use telarel_common::CompileOptions;

use crate::_types::hooks::options::OptionsArgs;
use crate::plugin::pluginable::SharedPluginable;

/// Run the `options` hook chain.
///
/// Each `Some` output replaces the carried options, so the last plugin wins.
pub async fn options(
    plugins: &[SharedPluginable],
    options: CompileOptions,
) -> anyhow::Result<CompileOptions> {
    let mut current: CompileOptions = options;

    for plugin in plugins {
        let args: OptionsArgs<'_> = OptionsArgs { options: &current };

        if let Some(output) = plugin.call_options(&args).await? {
            current = output.options;
        }
    }

    Ok(current)
}
