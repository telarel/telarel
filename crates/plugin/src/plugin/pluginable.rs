use std::any::Any;
use std::borrow::Cow;
use std::fmt::{Debug, Formatter};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use telarel_common::{HookUsage, ResolvableOptions};

use crate::_types::context::{CommonPluginContext, PluginContext};
use crate::_types::hooks::end::EndArgs;
use crate::_types::hooks::finalize::FinalizeArgs;
use crate::_types::hooks::generate::{GenerateArgs, GenerateReturn};
use crate::_types::hooks::notify::NotifyReturn;
use crate::_types::hooks::parse::{ParseArgs, ParseReturn};
use crate::_types::hooks::prepare::PrepareArgs;
use crate::_types::hooks::start::StartArgs;
use crate::_types::hooks::transform::{TransformArgs, TransformReturn};
use crate::_types::order::PluginHookMeta;
use crate::PluginError;
use crate::plugin::Plugin;

/// A shared, object-safe plugin handle.
pub type SharedPluginable = Arc<dyn Pluginable>;

/// A boxed, `Send` hook future.
pub type HookFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// A shared, one-call transform batch.
pub type SharedTransformBatch = Arc<dyn TransformBatch>;

/// Internal capability for a plugin that can run a contiguous transform run in
/// one call.
///
/// This capability is not part of the public plugin contract. It exists so the
/// driver can collapse several JavaScript transform hooks into a single AST
/// boundary crossing.
pub trait TransformBatchable: Send + Sync {
    /// Build one batch over `members`, or `None` to use individual calls.
    /// Members are in settled transform order.
    fn make_transform_batch(
        &self,
        members: &[&dyn Pluginable],
    ) -> Option<SharedTransformBatch>;
}

/// Internal one-call transform batch.
///
/// The contract matches the single transform hook: `None` keeps the carried
/// AST, and `Some` replaces it for the rest of the chain.
pub trait TransformBatch: Send + Sync {
    /// Call the batch once with the carried AST.
    fn call_transform<'a>(
        &'a self,
        ctx: &'a PluginContext<'_>,
        args: TransformArgs,
    ) -> HookFuture<'a, TransformReturn>;
}

/// Arguments for the `options` hook: resolvable options plus the plugin list.
///
/// A returned value is a patch, not a replacement. Each `Some` option field
/// overrides the current value and each `None` keeps it. Returned plugins
/// append to the current list. Returning `None` keeps the args unchanged.
#[derive(Debug, Clone)]
pub struct OptionsArgs {
    /// Resolvable compile options.
    pub options: ResolvableOptions,
    /// Plugins to run, in order.
    ///
    /// The settled list may repeat an instance. A returned plugin joins it
    /// only when the same instance is not already present.
    pub plugins: Vec<SharedPluginable>,
}

/// The `options` hook return: an option patch, `None`, or an error.
///
/// A `Some` patch merges into the current args. `None` keeps them unchanged.
pub type OptionsReturn = Result<Option<OptionsArgs>, PluginError>;

/// Object-safe mirror of [`Plugin`].
///
/// Each hook is exposed as a `call_*` method that returns a boxed future. Thus
/// the driver can store heterogeneous plugins behind [`SharedPluginable`].
pub trait Pluginable: Any + Send + Sync + 'static {
    /// The plugin name.
    fn call_name(&self) -> Cow<'static, str>;

    /// Query which hooks the plugin implements.
    fn call_register_hook_usage(&self) -> HookUsage;

    /// Call the `options` hook. A returned bag patches the current bag. The
    /// pipeline uses a present field to override the settled value, keeps the
    /// value of an absent field, and appends returned plugins to the settled
    /// list.
    fn call_options<'a>(
        &'a self,
        ctx: &'a CommonPluginContext,
        args: &'a OptionsArgs,
    ) -> HookFuture<'a, OptionsReturn>;

    /// Ordering for the `options` hook.
    fn call_options_meta(&self) -> Option<PluginHookMeta> {
        None
    }

    /// Call the `start` hook. The hook is notify-only.
    fn call_start<'a>(
        &'a self,
        ctx: &'a PluginContext<'_>,
        args: &'a StartArgs,
    ) -> HookFuture<'a, NotifyReturn>;

    /// Ordering for the `start` hook.
    fn call_start_meta(&self) -> Option<PluginHookMeta> {
        None
    }

    /// Call the `parse` hook.
    fn call_parse<'a>(
        &'a self,
        ctx: &'a PluginContext<'_>,
        args: ParseArgs<'a>,
    ) -> HookFuture<'a, ParseReturn>;

    /// Ordering for the `parse` hook.
    fn call_parse_meta(&self) -> Option<PluginHookMeta> {
        None
    }

    /// Call the `prepare` hook. The hook is notify-only.
    fn call_prepare<'a>(
        &'a self,
        ctx: &'a PluginContext<'_>,
        args: PrepareArgs<'a>,
    ) -> HookFuture<'a, NotifyReturn>;

    /// Ordering for the `prepare` hook.
    fn call_prepare_meta(&self) -> Option<PluginHookMeta> {
        None
    }

    /// Call the `transform` hook.
    fn call_transform<'a>(
        &'a self,
        ctx: &'a PluginContext<'_>,
        args: TransformArgs,
    ) -> HookFuture<'a, TransformReturn>;

    /// Ordering for the `transform` hook.
    fn call_transform_meta(&self) -> Option<PluginHookMeta> {
        None
    }

    /// Internal batching capability for the `transform` hook.
    ///
    /// The default is not batchable. The driver only groups plugins that
    /// return `Some` from this method into one transform call.
    fn as_transform_batchable(&self) -> Option<&dyn TransformBatchable> {
        None
    }

    /// Call the `finalize` hook. The hook is notify-only.
    fn call_finalize<'a>(
        &'a self,
        ctx: &'a PluginContext<'_>,
        args: FinalizeArgs<'a>,
    ) -> HookFuture<'a, NotifyReturn>;

    /// Ordering for the `finalize` hook.
    fn call_finalize_meta(&self) -> Option<PluginHookMeta> {
        None
    }

    /// Call the `generate` hook.
    fn call_generate<'a>(
        &'a self,
        ctx: &'a PluginContext<'_>,
        args: GenerateArgs<'a>,
    ) -> HookFuture<'a, GenerateReturn>;

    /// Ordering for the `generate` hook.
    fn call_generate_meta(&self) -> Option<PluginHookMeta> {
        None
    }

    /// Call the `end` hook. The hook is notify-only.
    fn call_end<'a>(
        &'a self,
        ctx: &'a PluginContext<'_>,
        args: &'a EndArgs,
    ) -> HookFuture<'a, NotifyReturn>;

    /// Ordering for the `end` hook.
    fn call_end_meta(&self) -> Option<PluginHookMeta> {
        None
    }
}

impl<T: Plugin> Pluginable for T {
    fn call_name(&self) -> Cow<'static, str> {
        Plugin::name(self)
    }

    fn call_register_hook_usage(&self) -> HookUsage {
        Plugin::register_hook_usage(self)
    }

    fn call_options<'a>(
        &'a self,
        ctx: &'a CommonPluginContext,
        args: &'a OptionsArgs,
    ) -> HookFuture<'a, OptionsReturn> {
        Box::pin(Plugin::options(self, ctx, args))
    }

    fn call_options_meta(&self) -> Option<PluginHookMeta> {
        Plugin::options_meta(self)
    }

    fn call_start<'a>(
        &'a self,
        ctx: &'a PluginContext<'_>,
        args: &'a StartArgs,
    ) -> HookFuture<'a, NotifyReturn> {
        Box::pin(Plugin::start(self, ctx, args))
    }

    fn call_start_meta(&self) -> Option<PluginHookMeta> {
        Plugin::start_meta(self)
    }

    fn call_parse<'a>(
        &'a self,
        ctx: &'a PluginContext<'_>,
        args: ParseArgs<'a>,
    ) -> HookFuture<'a, ParseReturn> {
        Box::pin(Plugin::parse(self, ctx, args))
    }

    fn call_parse_meta(&self) -> Option<PluginHookMeta> {
        Plugin::parse_meta(self)
    }

    fn call_prepare<'a>(
        &'a self,
        ctx: &'a PluginContext<'_>,
        args: PrepareArgs<'a>,
    ) -> HookFuture<'a, NotifyReturn> {
        Box::pin(Plugin::prepare(self, ctx, args))
    }

    fn call_prepare_meta(&self) -> Option<PluginHookMeta> {
        Plugin::prepare_meta(self)
    }

    fn call_transform<'a>(
        &'a self,
        ctx: &'a PluginContext<'_>,
        args: TransformArgs,
    ) -> HookFuture<'a, TransformReturn> {
        Box::pin(Plugin::transform(self, ctx, args))
    }

    fn call_transform_meta(&self) -> Option<PluginHookMeta> {
        Plugin::transform_meta(self)
    }

    fn call_finalize<'a>(
        &'a self,
        ctx: &'a PluginContext<'_>,
        args: FinalizeArgs<'a>,
    ) -> HookFuture<'a, NotifyReturn> {
        Box::pin(Plugin::finalize(self, ctx, args))
    }

    fn call_finalize_meta(&self) -> Option<PluginHookMeta> {
        Plugin::finalize_meta(self)
    }

    fn call_generate<'a>(
        &'a self,
        ctx: &'a PluginContext<'_>,
        args: GenerateArgs<'a>,
    ) -> HookFuture<'a, GenerateReturn> {
        Box::pin(Plugin::generate(self, ctx, args))
    }

    fn call_generate_meta(&self) -> Option<PluginHookMeta> {
        Plugin::generate_meta(self)
    }

    fn call_end<'a>(
        &'a self,
        ctx: &'a PluginContext<'_>,
        args: &'a EndArgs,
    ) -> HookFuture<'a, NotifyReturn> {
        Box::pin(Plugin::end(self, ctx, args))
    }

    fn call_end_meta(&self) -> Option<PluginHookMeta> {
        Plugin::end_meta(self)
    }
}

impl Debug for dyn Pluginable {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        f.write_str(self.call_name().as_ref())
    }
}

fn pick<T: Clone>(
    patch: &Option<T>,
    base: &Option<T>,
) -> Option<T> {
    patch.clone().or_else(|| base.clone())
}

/// Merge a returned `options` patch into the settled args.
///
/// A `Some` option field in `patch` overrides the settled value and a `None`
/// keeps it. A returned plugin appends to the settled list only when the same
/// instance (`Arc::ptr_eq`) is not already present.
pub fn merge_options(
    base: &OptionsArgs,
    patch: &OptionsArgs,
) -> OptionsArgs {
    let mut plugins: Vec<SharedPluginable> = base.plugins.clone();

    for plugin in &patch.plugins {
        if !plugins.iter().any(|present| Arc::ptr_eq(present, plugin)) {
            plugins.push(Arc::clone(plugin));
        }
    }

    OptionsArgs {
        options: ResolvableOptions {
            cwd: pick(&patch.options.cwd, &base.options.cwd),
            file: pick(&patch.options.file, &base.options.file),
            language: pick(&patch.options.language, &base.options.language),
            source_type: pick(
                &patch.options.source_type,
                &base.options.source_type,
            ),
            parser: pick(&patch.options.parser, &base.options.parser),
            generator: pick(&patch.options.generator, &base.options.generator),
            output: pick(&patch.options.output, &base.options.output),
            source_map: pick(
                &patch.options.source_map,
                &base.options.source_map,
            ),
        },
        plugins,
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;
    use std::path::PathBuf;
    use std::sync::Arc;

    use telarel_common::{HookUsage, Language, ResolvableOptions};

    use crate::plugin::Plugin;

    use super::*;

    #[derive(Debug)]
    struct ProbePlugin;

    impl Plugin for ProbePlugin {
        fn name(&self) -> Cow<'static, str> {
            "probe".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Options
        }
    }

    #[test]
    fn test_constructs_options_args() {
        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(ProbePlugin)];

        let args: OptionsArgs = OptionsArgs {
            options: ResolvableOptions {
                file: Some("a.ts".into()),
                ..ResolvableOptions::default()
            },
            plugins: plugins.clone(),
        };

        assert_eq!(args.options.file.as_deref(), Some("a.ts"));
        assert_eq!(args.plugins.len(), 1);
    }

    #[test]
    fn test_options_args_clone_shares_plugin_handles() {
        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(ProbePlugin)];

        let args: OptionsArgs =
            OptionsArgs { options: ResolvableOptions::default(), plugins };

        let cloned: OptionsArgs = args.clone();

        assert!(Arc::ptr_eq(&args.plugins[0], &cloned.plugins[0]));
    }

    #[test]
    fn test_options_args_carry_no_code_field() {
        let args: OptionsArgs = OptionsArgs {
            options: ResolvableOptions {
                cwd: Some(PathBuf::from("/repo")),
                language: Some(Language::TS),
                ..ResolvableOptions::default()
            },
            plugins: vec![],
        };

        assert_eq!(args.options.cwd, Some(PathBuf::from("/repo")));
        assert_eq!(args.options.language, Some(Language::TS));
    }
}
