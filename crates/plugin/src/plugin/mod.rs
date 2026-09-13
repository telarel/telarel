pub mod pluginable;

use std::any::Any;
use std::borrow::Cow;
use std::fmt::Debug;
use std::future::Future;

use telarel_common::CompileContext;

use crate::_types::hooks::options::{OptionsArgs, OptionsOutput};
use crate::_types::hooks::post::PostArgs;
use crate::_types::hooks::pre::PreArgs;
use crate::_types::hooks::transform::{TransformArgs, TransformReturn};

/// The Rust plugin trait.
pub trait Plugin: Any + Debug + Send + Sync + 'static {
    /// The plugin name.
    fn name(&self) -> Cow<'static, str>;

    /// Run the `options` hook; may replace the compile options.
    fn options<'a>(
        &'a self,
        _args: &'a OptionsArgs<'_>,
    ) -> impl Future<Output = anyhow::Result<Option<OptionsOutput>>> + Send
    {
        async { Ok(None) }
    }

    /// Run the `pre` hook, before the transform chain.
    fn pre<'a>(
        &'a self,
        _ctx: &'a CompileContext<'_>,
        _args: &'a PreArgs<'_>,
    ) -> impl Future<Output = anyhow::Result<()>> + Send {
        async { Ok(()) }
    }

    /// Run the `transform` hook.
    fn transform<'a>(
        &'a self,
        _ctx: &'a CompileContext<'a>,
        _args: &'a TransformArgs<'a>,
    ) -> impl Future<Output = TransformReturn<'a>> {
        async { Ok(None) }
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
