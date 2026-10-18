use anyhow::Context;
use telarel_common::Ast;

use crate::_types::context::PluginContext;
use crate::_types::hooks::transform::{TransformArgs, TransformOutput};
use crate::plugin::pluginable::SharedPluginable;

/// The carried AST through the `transform` fold.
///
/// Starts as a borrow of the caller's AST so the common "no plugin changed
/// anything" case never clones. The first `Some(output)` replaces the borrow
/// with the plugin's owned AST, and later plugins see that.
enum Carried<'a> {
    Borrowed(&'a Ast),
    Owned(Ast),
}

impl Carried<'_> {
    fn as_ref(&self) -> &Ast {
        match self {
            | Carried::Borrowed(ast) => ast,
            | Carried::Owned(ast) => ast,
        }
    }
}

/// Run the `transform` hook chain and return the last `Some` output, or
/// `None` when no plugin changed the AST.
pub async fn transform<'a>(
    plugins: &'a [SharedPluginable],
    ctx: &'a PluginContext<'_>,
    args: TransformArgs<'a>,
) -> anyhow::Result<Option<TransformOutput>> {
    let mut current: Carried<'a> = Carried::Borrowed(args.ast);

    let mut changed: bool = false;

    for plugin in plugins {
        let next: Option<TransformOutput> = plugin
            .call_transform(ctx, TransformArgs { ast: current.as_ref() })
            .await
            .with_context(|| format!("`{}` transform", plugin.call_name()))?;

        if let Some(output) = next {
            current = Carried::Owned(output.ast);

            changed = true;
        }
    }

    if !changed {
        return Ok(None);
    }

    let ast: Ast = match current {
        | Carried::Borrowed(ast) => ast.clone(),
        | Carried::Owned(ast) => ast,
    };

    Ok(Some(TransformOutput { ast }))
}
