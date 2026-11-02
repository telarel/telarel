use telarel_common::Ast;

use crate::_types::context::PluginContext;
use crate::_types::hooks::transform::{TransformArgs, TransformOutput};
use crate::PluginError;
use crate::plugin::pluginable::{
    Pluginable, SharedPluginable, SharedTransformBatch, TransformBatchable,
};

fn apply_transform_output(
    current: &mut Ast,
    changed: &mut bool,
    next: Option<TransformOutput>,
) {
    if let Some(output) = next {
        *current = output.ast;

        *changed = true;
    }
}

/// Ask the first run member to build one batch over the whole run.
///
/// The first member is batchable by construction, so a missing batchable
/// capability means there is nothing to batch.
fn make_batch(run: &[SharedPluginable]) -> Option<SharedTransformBatch> {
    let members: Vec<&dyn Pluginable> =
        run.iter().map(|plugin| plugin.as_ref()).collect();

    run.first().and_then(|plugin| plugin.as_transform_batchable()).and_then(
        |batchable: &dyn TransformBatchable| {
            batchable.make_transform_batch(&members)
        },
    )
}

/// Run the `transform` hook chain and return the last `Some` output, or
/// `None` when no plugin changed the AST.
///
/// The fold owns the carried AST. Each hook receives an O(1) clone. `None`
/// leaves the current AST untouched, and `Some` replaces it. A hook that
/// mutates through `Ast::with_mut` copies on write because the fold still owns
/// the current handle.
///
/// Maximal contiguous runs of plugins that are [`TransformBatchable`] share one
/// batch call. A run of one, or a batch construction that returns `None`, uses
/// the single-hook path. Non-batchable plugins break the run.
pub async fn transform(
    plugins: &[SharedPluginable],
    ctx: &PluginContext<'_>,
    args: TransformArgs,
) -> Result<Option<TransformOutput>, PluginError> {
    let mut current: Ast = args.ast;

    let mut changed: bool = false;

    let mut index: usize = 0;

    while index < plugins.len() {
        if plugins[index].as_transform_batchable().is_none() {
            let next: Option<TransformOutput> = plugins[index]
                .call_transform(ctx, TransformArgs { ast: current.clone() })
                .await?;

            apply_transform_output(&mut current, &mut changed, next);

            index += 1;

            continue;
        }

        let start: usize = index;

        index += 1;

        while index < plugins.len()
            && plugins[index].as_transform_batchable().is_some()
        {
            index += 1;
        }

        let run: &[SharedPluginable] = &plugins[start..index];

        if run.len() > 1
            && let Some(batch) = make_batch(run)
        {
            let next: Option<TransformOutput> = batch
                .call_transform(ctx, TransformArgs { ast: current.clone() })
                .await?;

            apply_transform_output(&mut current, &mut changed, next);

            continue;
        }

        for plugin in run {
            let next: Option<TransformOutput> = plugin
                .call_transform(ctx, TransformArgs { ast: current.clone() })
                .await?;

            apply_transform_output(&mut current, &mut changed, next);
        }
    }

    if !changed {
        return Ok(None);
    }

    Ok(Some(TransformOutput { ast: current }))
}
