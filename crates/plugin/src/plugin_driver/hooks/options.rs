use telarel_common::CompileOptions;

use crate::plugin::pluginable::SharedPluginable;

/// Run the `options` hook chain.
///
/// Each `Some` update is merged onto the carried options field-by-field
/// (`None` fields keep their current values), so the last plugin wins.
pub async fn options(
    plugins: &[SharedPluginable],
    options: CompileOptions,
) -> anyhow::Result<CompileOptions> {
    let mut current: CompileOptions = options;

    for plugin in plugins {
        if let Some(update) = plugin.call_options(&current).await? {
            if let Some(cwd) = update.cwd {
                current.cwd = cwd;
            }

            if let Some(file) = update.file {
                current.file = file;
            }

            if let Some(code) = update.code {
                current.code = code;
            }
        }
    }

    Ok(current)
}
