use telarel_common::Ast;

use crate::_types::context::PluginContext;
use crate::_types::hooks::transform::{TransformArgs, TransformOutput};
use crate::PluginError;
use crate::plugin::pluginable::SharedPluginable;

/// Run the `transform` hook chain and return the last `Some` output, or
/// `None` when no plugin changed the AST.
///
/// The fold owns the carried AST. Each hook receives an O(1) clone; `None`
/// leaves the current AST untouched, and `Some` replaces it. A hook that
/// mutates through `Ast::with_mut` copies on write because the fold still owns
/// the current handle.
pub async fn transform(
    plugins: &[SharedPluginable],
    ctx: &PluginContext<'_>,
    args: TransformArgs,
) -> Result<Option<TransformOutput>, PluginError> {
    let mut current: Ast = args.ast;

    let mut changed: bool = false;

    for plugin in plugins {
        let next: Option<TransformOutput> = plugin
            .call_transform(ctx, TransformArgs { ast: current.clone() })
            .await?;

        if let Some(output) = next {
            current = output.ast;

            changed = true;
        }
    }

    if !changed {
        return Ok(None);
    }

    Ok(Some(TransformOutput { ast: current }))
}
