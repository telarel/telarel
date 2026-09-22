pub mod pluginable;

use std::any::Any;
use std::borrow::Cow;
use std::fmt::Debug;
use std::future::Future;
use std::sync::Arc;

use telarel_common::{CompileContext, CompileOptions, HookUsage};

use crate::_types::hooks::post::PostArgs;
use crate::_types::hooks::pre::PreArgs;
use crate::_types::hooks::transform::{TransformArgs, TransformReturn};
use crate::plugin::pluginable::SharedPluginable;

/// The Rust plugin trait.
pub trait Plugin: Any + Debug + Send + Sync + 'static {
    /// Wrap a sized plugin into a shared, object-safe handle.
    fn new_shared(plugin: Self) -> SharedPluginable
    where
        Self: Sized,
    {
        Arc::new(plugin)
    }

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

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use telarel_common::HookUsage;

    use crate::plugin::pluginable::SharedPluginable;

    use super::*;

    #[derive(Debug)]
    struct ProbePlugin;

    impl Plugin for ProbePlugin {
        fn name(&self) -> Cow<'static, str> {
            "probe".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Pre
        }
    }

    #[test]
    fn test_new_shared_wraps_plugin() {
        let shared: SharedPluginable = Plugin::new_shared(ProbePlugin);

        assert_eq!(shared.call_name(), "probe");
        assert!(shared.call_register_hook_usage().contains(HookUsage::Pre));
    }
}
