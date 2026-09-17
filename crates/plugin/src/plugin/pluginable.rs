use std::any::Any;
use std::borrow::Cow;
use std::fmt::{Debug, Formatter};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use telarel_common::{CompileContext, HookUsage};

use crate::_types::hooks::options::{OptionsArgs, OptionsOutput};
use crate::_types::hooks::post::PostArgs;
use crate::_types::hooks::pre::PreArgs;
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
    fn call_hook_usage(&self) -> HookUsage;

    /// Call the `options` hook.
    fn call_options<'a>(
        &'a self,
        args: &'a OptionsArgs<'_>,
    ) -> HookFuture<'a, anyhow::Result<Option<OptionsOutput>>>;

    /// Call the `pre` hook.
    fn call_pre<'a>(
        &'a self,
        ctx: &'a CompileContext<'_>,
        args: &'a PreArgs<'_>,
    ) -> HookFuture<'a, anyhow::Result<()>>;

    /// Call the `transform` hook.
    fn call_transform<'a>(
        &'a self,
        ctx: &'a CompileContext<'a>,
        args: &'a TransformArgs<'a>,
    ) -> LocalHookFuture<'a, TransformReturn<'a>>;

    /// Call the `post` hook.
    fn call_post<'a>(
        &'a self,
        ctx: &'a CompileContext<'_>,
        args: &'a PostArgs<'_>,
    ) -> HookFuture<'a, anyhow::Result<()>>;
}

impl<T: Plugin> Pluginable for T {
    fn call_name(&self) -> Cow<'static, str> {
        Plugin::name(self)
    }

    fn call_hook_usage(&self) -> HookUsage {
        Plugin::hook_usage(self)
    }

    fn call_options<'a>(
        &'a self,
        args: &'a OptionsArgs<'_>,
    ) -> HookFuture<'a, anyhow::Result<Option<OptionsOutput>>> {
        Box::pin(Plugin::options(self, args))
    }

    fn call_pre<'a>(
        &'a self,
        ctx: &'a CompileContext<'_>,
        args: &'a PreArgs<'_>,
    ) -> HookFuture<'a, anyhow::Result<()>> {
        Box::pin(Plugin::pre(self, ctx, args))
    }

    fn call_transform<'a>(
        &'a self,
        ctx: &'a CompileContext<'a>,
        args: &'a TransformArgs<'a>,
    ) -> LocalHookFuture<'a, TransformReturn<'a>> {
        Box::pin(Plugin::transform(self, ctx, args))
    }

    fn call_post<'a>(
        &'a self,
        ctx: &'a CompileContext<'_>,
        args: &'a PostArgs<'_>,
    ) -> HookFuture<'a, anyhow::Result<()>> {
        Box::pin(Plugin::post(self, ctx, args))
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
