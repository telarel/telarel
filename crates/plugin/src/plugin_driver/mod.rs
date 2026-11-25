pub mod hooks;

use std::sync::Arc;

use telarel_common::{
    Ast, AstResult, CodeResult, CompileError, Generator, HookUsage, Parser,
    ResolvedGeneratorOptions, ResolvedParserOptions,
};

use crate::_types::context::PluginContext;
use crate::_types::hooks::end::EndArgs;
use crate::_types::hooks::finalize::FinalizeArgs;
use crate::_types::hooks::prepare::PrepareArgs;
use crate::_types::hooks::start::StartArgs;
use crate::_types::hooks::transform::{TransformArgs, TransformOutput};
use crate::_types::order::{PluginHookMeta, PluginOrder};
use crate::PluginError;
use crate::plugin::pluginable::SharedPluginable;

/// Order plugins by their per-hook meta: `[pre, normal, post]`, stable within a
/// bucket. A plugin whose hook meta is `None`, or whose meta carries
/// `order: None`, lands in the normal bucket.
pub fn sort_plugins_by_hook_meta(
    plugins: &[SharedPluginable],
    get_hook_meta: fn(&SharedPluginable) -> Option<PluginHookMeta>,
) -> Vec<SharedPluginable> {
    let mut pre: Vec<SharedPluginable> = Vec::new();
    let mut normal: Vec<SharedPluginable> = Vec::new();
    let mut post: Vec<SharedPluginable> = Vec::new();

    for plugin in plugins {
        match get_hook_meta(plugin).and_then(|meta| meta.order) {
            | Some(PluginOrder::Pre) => pre.push(Arc::clone(plugin)),
            | Some(PluginOrder::Post) => post.push(Arc::clone(plugin)),
            | None => normal.push(Arc::clone(plugin)),
        }
    }

    [pre, normal, post].concat()
}

/// Drives plugins through the hooks, each ordered by its per-hook meta
/// (`[pre, normal, post]`, stable within a bucket).
pub struct PluginDriver {
    usage: HookUsage,
    settled_names: Vec<String>,
    start_plugins: Vec<SharedPluginable>,
    parse_plugins: Vec<SharedPluginable>,
    prepare_plugins: Vec<SharedPluginable>,
    transform_plugins: Vec<SharedPluginable>,
    finalize_plugins: Vec<SharedPluginable>,
    generate_plugins: Vec<SharedPluginable>,
    end_plugins: Vec<SharedPluginable>,
}

impl PluginDriver {
    /// Create a driver from plugins in registration order.
    ///
    /// `start` and `end` run on every settled plugin. The driver partitions
    /// `parse`, `prepare`, `transform`, `finalize`, and `generate` by declared
    /// [`HookUsage`]. `parse` and `generate` are first-win, and `transform`
    /// carries the last `Some`. `prepare` and `finalize` are notify-only
    /// observers. `options` is not a driver partition. It is the fixpoint
    /// runner ([`crate::options_fixpoint`]).
    pub fn new(plugins: Vec<SharedPluginable>) -> Self {
        let mut usage: HookUsage = HookUsage::default();

        let mut parse_plugins: Vec<SharedPluginable> = Vec::new();
        let mut prepare_plugins: Vec<SharedPluginable> = Vec::new();
        let mut transform_plugins: Vec<SharedPluginable> = Vec::new();
        let mut finalize_plugins: Vec<SharedPluginable> = Vec::new();
        let mut generate_plugins: Vec<SharedPluginable> = Vec::new();

        for plugin in &plugins {
            let declared: HookUsage = plugin.call_register_hook_usage();

            usage |= declared;

            if declared.contains(HookUsage::Parse) {
                parse_plugins.push(Arc::clone(plugin));
            }

            if declared.contains(HookUsage::Prepare) {
                prepare_plugins.push(Arc::clone(plugin));
            }

            if declared.contains(HookUsage::Transform) {
                transform_plugins.push(Arc::clone(plugin));
            }

            if declared.contains(HookUsage::Finalize) {
                finalize_plugins.push(Arc::clone(plugin));
            }

            if declared.contains(HookUsage::Generate) {
                generate_plugins.push(Arc::clone(plugin));
            }
        }

        let start_plugins: Vec<SharedPluginable> =
            sort_plugins_by_hook_meta(&plugins, |plugin| {
                plugin.call_start_meta()
            });

        let parse_plugins: Vec<SharedPluginable> =
            sort_plugins_by_hook_meta(&parse_plugins, |plugin| {
                plugin.call_parse_meta()
            });

        let prepare_plugins: Vec<SharedPluginable> =
            sort_plugins_by_hook_meta(&prepare_plugins, |plugin| {
                plugin.call_prepare_meta()
            });

        let transform_plugins: Vec<SharedPluginable> =
            sort_plugins_by_hook_meta(&transform_plugins, |plugin| {
                plugin.call_transform_meta()
            });

        let finalize_plugins: Vec<SharedPluginable> =
            sort_plugins_by_hook_meta(&finalize_plugins, |plugin| {
                plugin.call_finalize_meta()
            });

        let generate_plugins: Vec<SharedPluginable> =
            sort_plugins_by_hook_meta(&generate_plugins, |plugin| {
                plugin.call_generate_meta()
            });

        let end_plugins: Vec<SharedPluginable> =
            sort_plugins_by_hook_meta(&plugins, |plugin| {
                plugin.call_end_meta()
            });

        let settled_names: Vec<String> = plugins
            .iter()
            .map(|plugin| plugin.call_name().into_owned())
            .collect();

        Self {
            usage,
            settled_names,
            start_plugins,
            parse_plugins,
            prepare_plugins,
            transform_plugins,
            finalize_plugins,
            generate_plugins,
            end_plugins,
        }
    }

    /// The aggregate hook usage across all plugins.
    pub fn usage(&self) -> HookUsage {
        self.usage
    }

    /// The settled plugin names in registration order. Duplicates are
    /// preserved.
    pub fn settled_names(&self) -> &[String] {
        &self.settled_names
    }

    /// Run the `start` hook on EVERY settled plugin (not partitioned by
    /// usage). The first error aborts. The hook is notify-only.
    pub async fn start(
        &self,
        ctx: &PluginContext<'_>,
        args: &StartArgs,
    ) -> Result<(), PluginError> {
        hooks::start::start(&self.start_plugins, ctx, args).await
    }

    /// Run the `parse` hook over the `parse` partition.
    ///
    /// First-win: the first `Ok(Some)` result wins, and the driver skips the
    /// remaining hooks. `Ok(None)` defers to the next plugin. The first error
    /// aborts. When every hook defers, `builtin` runs and its raw error is
    /// returned.
    pub async fn parse(
        &self,
        ctx: &PluginContext<'_>,
        code: &str,
        options: &ResolvedParserOptions,
        builtin: &Parser,
    ) -> Result<AstResult, CompileError> {
        hooks::parse::parse(&self.parse_plugins, ctx, code, options, builtin)
            .await
    }

    /// Run the `prepare` hook over the `prepare` partition in meta-ranked
    /// order, as a read-only observer of the parsed AST. The first error
    /// aborts. The driver ignores return values.
    pub async fn prepare<'a>(
        &'a self,
        ctx: &'a PluginContext<'_>,
        args: PrepareArgs<'a>,
    ) -> Result<(), PluginError> {
        hooks::prepare::prepare(&self.prepare_plugins, ctx, args).await
    }

    /// Run the `transform` hook chain. `Some` carries the last returned AST,
    /// and `None` means the AST is unchanged.
    ///
    /// Maximal contiguous runs of batchable plugins share one batch call. A
    /// run of one, or a run whose batch construction fails, uses the
    /// single-hook path.
    pub async fn transform(
        &self,
        ctx: &PluginContext<'_>,
        args: TransformArgs,
    ) -> Result<Option<TransformOutput>, PluginError> {
        hooks::transform::transform(&self.transform_plugins, ctx, args).await
    }

    /// Run the `finalize` hook over the `finalize` partition in meta-ranked
    /// order, as a read-only observer of the final AST. The first error
    /// aborts. The driver ignores return values.
    pub async fn finalize<'a>(
        &'a self,
        ctx: &'a PluginContext<'_>,
        args: FinalizeArgs<'a>,
    ) -> Result<(), PluginError> {
        hooks::finalize::finalize(&self.finalize_plugins, ctx, args).await
    }

    /// Run the `generate` hook over the `generate` partition.
    ///
    /// First-win: the first `Ok(Some)` result wins, and the driver skips the
    /// remaining hooks. `Ok(None)` defers to the next plugin. The first error
    /// aborts. When every hook defers, `builtin` runs and its raw error is
    /// returned.
    pub async fn generate(
        &self,
        ctx: &PluginContext<'_>,
        ast: &Ast,
        options: &ResolvedGeneratorOptions,
        builtin: &Generator<'_>,
    ) -> Result<CodeResult, CompileError> {
        hooks::generate::generate(
            &self.generate_plugins,
            ctx,
            ast,
            options,
            builtin,
        )
        .await
    }

    /// Run the `end` hook on EVERY settled plugin (not partitioned by usage).
    /// Every hook runs even after one fails, and the driver returns the first
    /// error after the loop. The hook is notify-only.
    pub async fn end(
        &self,
        ctx: &PluginContext<'_>,
        args: &EndArgs,
    ) -> Result<(), PluginError> {
        hooks::end::end(&self.end_plugins, ctx, args).await
    }
}

// The house style for the hook impls is the explicit RPITIT form (never
// `async fn`), so the `manual_async_fn` lint is suppressed here.
#[cfg(test)]
#[allow(clippy::manual_async_fn)]
mod tests;
