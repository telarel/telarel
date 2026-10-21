use telarel_common::ResolvableOptions;

use crate::PluginError;
use crate::plugin::pluginable::SharedPluginable;

/// Arguments for the `options` hook: resolvable options plus the plugin list.
///
/// The `options` hook returns args to replace the whole set — fields not
/// present are dropped (struct-level replace); returning `None` keeps it.
#[derive(Debug, Clone)]
pub struct OptionsArgs {
    /// Resolvable compile options.
    pub options: ResolvableOptions,
    /// Plugins to run, in order; duplicates are allowed.
    pub plugins: Vec<SharedPluginable>,
}

/// The `options` hook return: replacement args, `None`, or an error.
pub type OptionsReturn = Result<Option<OptionsArgs>, PluginError>;

#[cfg(test)]
mod tests {
    use std::borrow::Cow;
    use std::path::PathBuf;
    use std::sync::Arc;

    use telarel_common::{HookUsage, Language, ResolvableOptions};

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
        // The options payload has no `code`; language/source-type are optional.
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
