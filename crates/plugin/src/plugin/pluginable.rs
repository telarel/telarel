use std::any::Any;
use std::borrow::Cow;
use std::fmt::{Debug, Formatter};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use telarel_common::HookUsage;

use crate::_types::context::{CommonPluginContext, PluginContext};
use crate::_types::hooks::compile_end::CompileEndArgs;
use crate::_types::hooks::compile_start::CompileStartArgs;
use crate::_types::hooks::finalize::{FinalizeArgs, FinalizeReturn};
use crate::_types::hooks::notify::NotifyReturn;
use crate::_types::hooks::options::{OptionsArgs, OptionsReturn};
use crate::_types::hooks::prepare::{PrepareArgs, PrepareReturn};
use crate::_types::hooks::transform::{TransformArgs, TransformReturn};
use crate::plugin::Plugin;

/// A shared, object-safe plugin handle.
pub type SharedPluginable = Arc<dyn Pluginable>;

/// A boxed, `Send` hook future.
pub type HookFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// A boxed, non-`Send` hook future.
pub type LocalHookFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;

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

    /// Call the `compile_start` hook; notify-only.
    fn call_compile_start<'a>(
        &'a self,
        ctx: &'a PluginContext<'_>,
        args: &'a CompileStartArgs,
    ) -> HookFuture<'a, NotifyReturn>;

    /// Call the `prepare` hook.
    fn call_prepare<'a>(
        &'a self,
        ctx: &'a PluginContext<'_>,
        args: &'a PrepareArgs<'_>,
    ) -> HookFuture<'a, PrepareReturn>;

    /// Call the `transform` hook.
    fn call_transform<'a, 'ast: 'a>(
        &'a self,
        ctx: &'a PluginContext<'a>,
        args: TransformArgs<'ast>,
    ) -> LocalHookFuture<'a, TransformReturn<'ast>>;

    /// Call the `finalize` hook.
    fn call_finalize<'a>(
        &'a self,
        ctx: &'a PluginContext<'_>,
        args: &'a FinalizeArgs<'_>,
    ) -> HookFuture<'a, FinalizeReturn>;

    /// Call the `compile_end` hook; notify-only.
    fn call_compile_end<'a>(
        &'a self,
        ctx: &'a PluginContext<'_>,
        args: &'a CompileEndArgs,
    ) -> HookFuture<'a, NotifyReturn>;
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

    fn call_compile_start<'a>(
        &'a self,
        ctx: &'a PluginContext<'_>,
        args: &'a CompileStartArgs,
    ) -> HookFuture<'a, NotifyReturn> {
        Box::pin(Plugin::compile_start(self, ctx, args))
    }

    fn call_prepare<'a>(
        &'a self,
        ctx: &'a PluginContext<'_>,
        args: &'a PrepareArgs<'_>,
    ) -> HookFuture<'a, PrepareReturn> {
        Box::pin(Plugin::prepare(self, ctx, args))
    }

    fn call_transform<'a, 'ast: 'a>(
        &'a self,
        ctx: &'a PluginContext<'a>,
        args: TransformArgs<'ast>,
    ) -> LocalHookFuture<'a, TransformReturn<'ast>> {
        Box::pin(Plugin::transform(self, ctx, args))
    }

    fn call_finalize<'a>(
        &'a self,
        ctx: &'a PluginContext<'_>,
        args: &'a FinalizeArgs<'_>,
    ) -> HookFuture<'a, FinalizeReturn> {
        Box::pin(Plugin::finalize(self, ctx, args))
    }

    fn call_compile_end<'a>(
        &'a self,
        ctx: &'a PluginContext<'_>,
        args: &'a CompileEndArgs,
    ) -> HookFuture<'a, NotifyReturn> {
        Box::pin(Plugin::compile_end(self, ctx, args))
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
