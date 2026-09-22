pub mod pluginable;

use std::any::Any;
use std::borrow::Cow;
use std::fmt::Debug;
use std::future::Future;

use telarel_common::{CompileContext, CompileOptions, HookUsage};

use crate::_types::hooks::post::PostArgs;
use crate::_types::hooks::pre::PreArgs;
use crate::_types::hooks::transform::{TransformArgs, TransformReturn};

/// The Rust plugin trait.
pub trait Plugin: Any + Debug + Send + Sync + 'static {
    /// The plugin name.
    fn name(&self) -> Cow<'static, str>;

    /// Which hooks this plugin implements; hooks not declared are never called.
    fn register_hook_usage(&self) -> HookUsage;

    /// Run the `options` hook; mutate `options` in place. Plugins assign
    /// the fields they want to change and return `Ok(())`.
    fn options<'a>(
        &'a self,
        _options: &'a mut CompileOptions,
    ) -> impl Future<Output = anyhow::Result<()>> + Send {
        async { Ok(()) }
    }

    /// Run the `pre` hook, before the transform chain.
    fn pre<'a>(
        &'a self,
        _ctx: &'a CompileContext<'_>,
        _args: &'a PreArgs<'_>,
    ) -> impl Future<Output = anyhow::Result<()>> + Send {
        async { Ok(()) }
    }

    /// Run the `transform` hook; mutate `args.program` in place. A plugin
    /// may swap the entire root by assigning a freshly parsed `Program`
    /// over `*args.program`.
    fn transform<'a, 'ast>(
        &'a self,
        _ctx: &'a CompileContext<'a>,
        _args: TransformArgs<'a, 'ast>,
    ) -> impl Future<Output = TransformReturn> {
        async { Ok(()) }
    }

    /// Run the `post` hook, after the transform chain.
    fn post<'a>(
        &'a self,
        _ctx: &'a CompileContext<'_>,
        _args: &'a PostArgs<'_>,
    ) -> impl Future<Output = anyhow::Result<()>> + Send {
        async { Ok(()) }
    }
}
