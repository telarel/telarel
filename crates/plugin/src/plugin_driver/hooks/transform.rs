use anyhow::Context;
use oxc::ast::ast::Program;

use crate::_types::context::PluginContext;
use crate::_types::hooks::transform::{TransformArgs, TransformOutput};
use crate::plugin::pluginable::SharedPluginable;

/// Run the `transform` hook chain.
///
/// The chain is a fold over a carried program reference: each plugin
/// receives the program carried by the previous one and may return a
/// replacement. `Some(output)` replaces the carried program; `None` keeps
/// it. The returned value carries the LAST `Some`-returned program (a later
/// `None` plugin cannot change it, since a `None` result is by definition
/// the program that was carried in), so callers can codegen it directly.
/// `None` means no plugin changed anything.
pub async fn transform<'a, 'ast: 'a>(
    plugins: &'a [SharedPluginable],
    ctx: &'a PluginContext<'a>,
    args: &TransformArgs<'ast>,
) -> anyhow::Result<Option<TransformOutput<'ast>>> {
    let mut current: &'ast Program<'ast> = args.ast;

    let mut last_some: Option<&'ast Program<'ast>> = None;

    for plugin in plugins {
        let hook_args: TransformArgs<'ast> =
            TransformArgs { allocator: args.allocator, ast: current };

        let next: Option<TransformOutput<'ast>> = plugin
            .call_transform(ctx, hook_args)
            .await
            .with_context(|| format!("`{}` transform", plugin.call_name()))?;

        if let Some(output) = next {
            current = output.ast;

            last_some = Some(output.ast);
        }
    }

    Ok(last_some.map(|ast| TransformOutput { ast }))
}
