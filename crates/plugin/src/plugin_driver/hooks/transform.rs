use anyhow::Context;
use oxc::allocator::CloneIn;
use oxc::ast::ast::Program;
use telarel_common::CompileContext;

use crate::_types::hooks::transform::{TransformArgs, TransformReturn};
use crate::plugin::pluginable::SharedPluginable;

/// Run the `transform` hook chain.
///
/// Each `Some` output replaces the program carried into the next plugin.
pub async fn transform<'a>(
    plugins: &'a [SharedPluginable],
    ctx: &'a CompileContext<'a>,
    args: &'a TransformArgs<'a>,
) -> anyhow::Result<Option<Program<'a>>> {
    let mut current: Option<&'a Program<'a>> = None;

    for plugin in plugins {
        let hook_args: &'a TransformArgs<'a> =
            args.allocator.alloc(TransformArgs {
                allocator: args.allocator,
                file: args.file,
                program: current.unwrap_or(args.program),
            });

        let result: TransformReturn<'a> =
            plugin.call_transform(ctx, hook_args).await;

        if let Some(output) = result
            .with_context(|| format!("`{}` transform", plugin.call_name()))?
        {
            let placed: &'a Program<'a> = args.allocator.alloc(output.program);

            current = Some(placed);
        }
    }

    Ok(current.map(|program: &Program<'a>| program.clone_in(args.allocator)))
}
