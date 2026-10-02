use telarel_common::CompileOptions;

use crate::plugin::pluginable::SharedPluginable;

/// Arguments for the `options` hook: scalar options plus the plugin list.
///
/// The `options` hook returns args to replace the whole set — fields not
/// present are dropped (struct-level replace); returning `None` keeps it.
#[derive(Debug, Clone)]
pub struct OptionsArgs {
    /// Scalar compile options.
    pub options: CompileOptions,
    /// Plugins to run, in order; duplicates are allowed.
    pub plugins: Vec<SharedPluginable>,
}

/// The `options` hook return: replacement args, `None`, or an error.
pub type OptionsReturn = anyhow::Result<Option<OptionsArgs>>;

#[cfg(test)]
mod tests {
    use std::borrow::Cow;
    use std::sync::Arc;

    use telarel_common::{CompileOptions, HookUsage};

    use crate::plugin::Plugin;
    use crate::plugin::pluginable::SharedPluginable;

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
            options: CompileOptions {
                file: "a.ts".into(),
                code: "let a;".into(),
                ..CompileOptions::default()
            },
            plugins: plugins.clone(),
        };

        assert_eq!(args.options.file, "a.ts");
        assert_eq!(args.plugins.len(), 1);
    }

    #[test]
    fn test_options_args_clone_shares_plugin_handles() {
        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(ProbePlugin)];

        let args: OptionsArgs =
            OptionsArgs { options: CompileOptions::default(), plugins };

        let cloned: OptionsArgs = args.clone();

        assert!(Arc::ptr_eq(&args.plugins[0], &cloned.plugins[0]));
    }
}
