use std::sync::Arc;

use telarel_common::ResolvableOptions;

use crate::PluginError;
use crate::plugin::pluginable::SharedPluginable;

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

fn pick<T: Clone>(
    patch: &Option<T>,
    base: &Option<T>,
) -> Option<T> {
    patch.clone().or_else(|| base.clone())
}

/// Merge a returned `options` patch into the settled args.
///
/// A `Some` option field in `patch` overrides the settled value and a `None`
/// keeps it. A patch plugin appends to the settled list only when the same
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
