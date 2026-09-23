mod jsx;
mod target;
mod typescript;

use oxc::transformer::Engine;
use oxc::transformer::EngineTargets;
use oxc::transformer::TransformOptions as OxcTransformOptions;
use oxc_compat::Version;

pub use jsx::{JsxOptions, JsxRuntime};
pub use target::TransformTarget;
pub use typescript::TypeScriptOptions;

/// The transform plugin options.
#[derive(Debug, Clone, Default)]
pub struct TransformOptions {
    /// Empty means the ESNext default; targets win over an `oxc` layer env.
    pub targets: Vec<TransformTarget>,
    pub jsx: Option<JsxOptions>,
    pub typescript: Option<TypeScriptOptions>,
    /// Raw backend passthrough (base layer).
    pub oxc: Option<OxcTransformOptions>,
}

impl TransformOptions {
    /// Apply the compile context `cwd` onto resolved oxc options.
    ///
    /// The compile options `cwd` is the run-level source of truth, so it
    /// always overwrites the oxc base layer's `cwd`.
    pub(crate) fn apply_cwd(
        base: &mut OxcTransformOptions,
        cwd: &str,
    ) {
        base.cwd = std::path::PathBuf::from(cwd);
    }

    /// Build the `EngineTargets` from the resolved targets,
    /// or `None` when no targets are configured.
    pub(crate) fn engine_targets(
        targets: &[TransformTarget]
    ) -> Option<EngineTargets> {
        if targets.is_empty() {
            return None;
        }

        let mut engine_targets: EngineTargets = EngineTargets::default();

        for target in targets {
            let (engine, version): (Engine, Version) = target.resolve();

            engine_targets.insert(engine, version);
        }

        Some(engine_targets)
    }

    /// Resolve into the oxc transform options.
    pub(crate) fn resolve(&self) -> OxcTransformOptions {
        let mut base: OxcTransformOptions =
            self.oxc.clone().unwrap_or_default();

        if let Some(jsx) = &self.jsx {
            jsx::overlay_jsx(&mut base.jsx, jsx);
        }

        if let Some(typescript) = &self.typescript {
            typescript::overlay_typescript(&mut base.typescript, typescript);
        }

        if let Some(engine_targets) = Self::engine_targets(&self.targets) {
            base.env = oxc::transformer::EnvOptions::from(engine_targets);
        }

        base
    }
}

#[cfg(test)]
mod tests {
    use oxc::transformer::Engine;
    use oxc::transformer::EngineTargets;
    use oxc::transformer::JsxOptions as OxcJsxOptions;
    use oxc::transformer::TransformOptions as OxcTransformOptions;
    use oxc_compat::Version;

    use crate::options::jsx::{JsxOptions, JsxRuntime};
    use crate::options::target::TransformTarget;
    use crate::options::typescript::TypeScriptOptions;

    use super::*;

    fn oxc_base() -> OxcTransformOptions {
        OxcTransformOptions {
            jsx: OxcJsxOptions {
                pragma: Some(String::from("h")),
                ..OxcJsxOptions::default()
            },
            ..OxcTransformOptions::default()
        }
    }

    #[test]
    fn test_apply_cwd_sets_field() {
        let mut base: OxcTransformOptions = OxcTransformOptions::default();

        TransformOptions::apply_cwd(&mut base, "/repo");

        assert_eq!(base.cwd, std::path::PathBuf::from("/repo"));
    }

    #[test]
    fn test_engine_targets_empty_is_none() {
        let targets: Option<EngineTargets> =
            TransformOptions::engine_targets(&[]);

        assert!(targets.is_none());
    }

    #[test]
    fn test_engine_targets_single() {
        let targets: Option<EngineTargets> =
            TransformOptions::engine_targets(&[TransformTarget::Es2020]);

        let targets: EngineTargets = targets.expect("targets");

        assert_eq!(targets.get(&Engine::Es), Some(&Version(2020, 0, 0)));
    }

    #[test]
    fn test_engine_targets_multiple_merge() {
        let targets: Option<EngineTargets> =
            TransformOptions::engine_targets(&[
                TransformTarget::Es2020,
                TransformTarget::Chrome(91),
                TransformTarget::Node(14),
            ]);

        let targets: EngineTargets = targets.expect("targets");

        assert_eq!(targets.get(&Engine::Es), Some(&Version(2020, 0, 0)));
        assert_eq!(targets.get(&Engine::Chrome), Some(&Version(91, 0, 0)));
        assert_eq!(targets.get(&Engine::Node), Some(&Version(14, 0, 0)));
    }

    #[test]
    fn test_default_transform_options_is_empty() {
        let options: TransformOptions = TransformOptions::default();

        assert!(options.targets.is_empty());
        assert!(options.jsx.is_none());
        assert!(options.typescript.is_none());
        assert!(options.oxc.is_none());
    }

    #[test]
    fn test_resolve_defaults() {
        let options: TransformOptions = TransformOptions::default();

        let resolved: OxcTransformOptions = options.resolve();

        assert!(resolved.env.es2015.arrow_function.is_none());
        assert_eq!(
            resolved.jsx.runtime,
            oxc::transformer::JsxRuntime::Automatic
        );
        assert!(resolved.jsx.jsx_plugin);
    }

    #[test]
    fn test_resolve_targets_lower_env() {
        let options: TransformOptions = TransformOptions {
            targets: vec![TransformTarget::Es2016],
            ..TransformOptions::default()
        };

        let resolved: OxcTransformOptions = options.resolve();

        assert!(!resolved.env.es2016.exponentiation_operator);
        assert!(resolved.env.es2017.async_to_generator);
        assert!(resolved.env.es2020.optional_chaining);
    }

    #[test]
    fn test_resolve_esnext_keeps_env_default() {
        let options: TransformOptions = TransformOptions {
            targets: vec![TransformTarget::EsNext],
            ..TransformOptions::default()
        };

        let resolved: OxcTransformOptions = options.resolve();

        assert!(resolved.env.es2015.arrow_function.is_none());
        assert!(!resolved.env.es2017.async_to_generator);
    }

    #[test]
    fn test_resolve_oxc_base_preserved_without_agnostic() {
        let options: TransformOptions = TransformOptions {
            oxc: Some(OxcTransformOptions {
                jsx: OxcJsxOptions {
                    pragma: Some(String::from("h")),
                    ..OxcJsxOptions::default()
                },
                ..OxcTransformOptions::default()
            }),
            ..TransformOptions::default()
        };

        let resolved: OxcTransformOptions = options.resolve();

        assert_eq!(resolved.jsx.pragma.as_deref(), Some("h"));
    }

    #[test]
    fn test_resolve_oxc_env_kept_when_targets_empty() {
        let mut engine_targets: EngineTargets = EngineTargets::default();

        engine_targets.insert(Engine::Es, Version(2016, 0, 0));

        let env: oxc::transformer::EnvOptions =
            oxc::transformer::EnvOptions::from(engine_targets);

        let options: TransformOptions = TransformOptions {
            oxc: Some(OxcTransformOptions {
                env,
                ..OxcTransformOptions::default()
            }),
            ..TransformOptions::default()
        };

        let resolved: OxcTransformOptions = options.resolve();

        assert!(!resolved.env.es2016.exponentiation_operator);
        assert!(resolved.env.es2017.async_to_generator);
    }

    #[test]
    fn test_resolve_targets_win_over_oxc_env() {
        let mut engine_targets: EngineTargets = EngineTargets::default();

        engine_targets.insert(Engine::Es, Version(2016, 0, 0));

        let env: oxc::transformer::EnvOptions =
            oxc::transformer::EnvOptions::from(engine_targets);

        let options: TransformOptions = TransformOptions {
            targets: vec![TransformTarget::Es2015],
            oxc: Some(OxcTransformOptions {
                env,
                ..OxcTransformOptions::default()
            }),
            ..TransformOptions::default()
        };

        let resolved: OxcTransformOptions = options.resolve();

        assert!(resolved.env.es2016.exponentiation_operator);
        assert!(resolved.env.es2017.async_to_generator);
    }

    #[test]
    fn test_resolve_agnostic_wins_per_field_over_oxc() {
        let options: TransformOptions = TransformOptions {
            jsx: Some(JsxOptions {
                runtime: Some(JsxRuntime::Classic),
                ..JsxOptions::default()
            }),
            oxc: Some(OxcTransformOptions {
                jsx: OxcJsxOptions {
                    runtime: oxc::transformer::JsxRuntime::Automatic,
                    import_source: Some(String::from("preact")),
                    ..OxcJsxOptions::default()
                },
                ..OxcTransformOptions::default()
            }),
            ..TransformOptions::default()
        };

        let resolved: OxcTransformOptions = options.resolve();

        assert_eq!(resolved.jsx.runtime, oxc::transformer::JsxRuntime::Classic);
        assert_eq!(resolved.jsx.import_source.as_deref(), Some("preact"));
    }

    #[test]
    fn test_resolve_typescript_overlay() {
        let options: TransformOptions = TransformOptions {
            typescript: Some(TypeScriptOptions {
                optimize_enums: Some(true),
                jsx_pragma: Some(String::from("h")),
                ..TypeScriptOptions::default()
            }),
            oxc: Some(OxcTransformOptions {
                typescript: oxc::transformer::TypeScriptOptions {
                    only_remove_type_imports: true,
                    ..oxc::transformer::TypeScriptOptions::default()
                },
                ..OxcTransformOptions::default()
            }),
            ..TransformOptions::default()
        };

        let resolved: OxcTransformOptions = options.resolve();

        assert!(resolved.typescript.only_remove_type_imports);
        assert!(resolved.typescript.optimize_enums);
        assert_eq!(resolved.typescript.jsx_pragma, "h");
    }

    #[test]
    fn test_oxc_base_survives_clone() {
        let options: TransformOptions = TransformOptions {
            oxc: Some(oxc_base()),
            ..TransformOptions::default()
        };

        let resolved_oxc: OxcTransformOptions = options.oxc.expect("oxc layer");

        assert_eq!(resolved_oxc.jsx.pragma.as_deref(), Some("h"));
    }

    #[test]
    fn test_resolve_oxc_helper_loader_passthrough() {
        let options: TransformOptions = TransformOptions {
            oxc: Some(OxcTransformOptions {
                helper_loader: oxc::transformer::HelperLoaderOptions {
                    mode: oxc::transformer::HelperLoaderMode::External,
                    ..oxc::transformer::HelperLoaderOptions::default()
                },
                ..OxcTransformOptions::default()
            }),
            ..TransformOptions::default()
        };

        let resolved: OxcTransformOptions = options.resolve();

        assert!(matches!(
            resolved.helper_loader.mode,
            oxc::transformer::HelperLoaderMode::External
        ));
    }
}
