use anyhow::Context;
use telarel_common::CompileContext;

use crate::_types::hooks::transform::TransformArgs;
use crate::plugin::pluginable::SharedPluginable;

/// Run the `transform` hook chain.
///
/// Each plugin mutates the same program in place; mutations are visible to
/// the following plugins. The program stays rooted in the compile
/// allocator, so pointer stability is preserved across the chain.
pub async fn transform<'a, 'ast>(
    plugins: &'a [SharedPluginable],
    ctx: &'a CompileContext<'a>,
    args: &mut TransformArgs<'a, 'ast>,
) -> anyhow::Result<()> {
    for plugin in plugins {
        let hook_args: TransformArgs<'_, '_> = TransformArgs {
            allocator: args.allocator,
            file: args.file,
            program: &mut *args.program,
        };

        plugin
            .call_transform(ctx, hook_args)
            .await
            .with_context(|| format!("`{}` transform", plugin.call_name()))?;
    }

    Ok(())
}
