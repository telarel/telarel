use std::any::Any;
use std::borrow::Cow;
use std::fmt::{Debug, Formatter};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use telarel_common::HookUsage;

use crate::_types::context::{CommonPluginContext, PluginContext};
use crate::_types::hooks::end::EndArgs;
use crate::_types::hooks::finalize::FinalizeArgs;
use crate::_types::hooks::generate::{GenerateArgs, GenerateReturn};
use crate::_types::hooks::notify::NotifyReturn;
use crate::_types::hooks::options::{OptionsArgs, OptionsReturn};
use crate::_types::hooks::parse::{ParseArgs, ParseReturn};
use crate::_types::hooks::prepare::PrepareArgs;
use crate::_types::hooks::start::StartArgs;
use crate::_types::hooks::transform::{TransformArgs, TransformReturn};
use crate::_types::order::PluginHookMeta;
use crate::plugin::Plugin;

/// A shared, object-safe plugin handle.
pub type SharedPluginable = Arc<dyn Pluginable>;

/// A boxed, `Send` hook future.
pub type HookFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Object-safe mirror of [`Plugin`].
///
/// Each hook is exposed as a `call_*` method returning a boxed future, so the
/// driver can store heterogeneous plugins behind [`SharedPluginable`].
pub trait Pluginable: Any + Send + Sync + 'static {
    /// The plugin name.
    fn call_name(&self) -> Cow<'static, str>;

    /// Query which hooks the plugin implements.
    fn call_register_hook_usage(&self) -> HookUsage;

    /// Call the `options` hook; a returned bag replaces the whole bag.
    fn call_options<'a>(
        &'a self,
        ctx: &'a CommonPluginContext,
        args: &'a OptionsArgs,
    ) -> HookFuture<'a, OptionsReturn>;

    /// Ordering for the `options` hook.
    fn call_options_meta(&self) -> Option<PluginHookMeta> {
        None
    }

    /// Call the `start` hook; notify-only.
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

    /// Call the `prepare` hook; notify-only.
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

    /// Call the `finalize` hook; notify-only.
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

    /// Call the `end` hook; notify-only.
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
