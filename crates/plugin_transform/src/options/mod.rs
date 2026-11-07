pub mod define;
pub mod helper_loader;
pub mod inject;
pub mod jsx;
pub mod output;
pub mod target;
pub mod typescript;

use std::path::Path;

use oxc::diagnostics::Diagnostics;
use oxc::transformer::Engine;
use oxc::transformer::EngineTargets;
use oxc::transformer::TransformOptions as OxcTransformOptions;
use oxc_compat::Version;
use oxc_transformer_plugins::{
    InjectGlobalVariablesConfig, ReplaceGlobalDefinesConfig,
};
use telarel_common::FILE_DEFAULT;
use telarel_common::Language;
use telarel_common::ResolvableOptions;
use telarel_common::infer_language;

use crate::options::define::DefineOptions;
use crate::options::helper_loader::HelperLoaderOptions;
use crate::options::helper_loader::ResolvedHelpers;
use crate::options::helper_loader::overlay_helper_loader;
use crate::options::helper_loader::resolve_helpers;
use crate::options::inject::InjectOptions;
use crate::options::jsx::JsxOptions;
use crate::options::jsx::JsxRuntime;
use crate::options::jsx::effective_runtime;
use crate::options::jsx::overlay_jsx;
use crate::options::output::derive_output_file;
use crate::options::target::TransformTarget;
use crate::options::typescript::TypeScriptOptions;
use crate::options::typescript::overlay_typescript;

/// Configures the transform passes and their inputs.
#[derive(Debug, Clone, Default)]
pub struct TransformOptions {
    /// Empty means the ESNext default. Targets win over an `oxc` layer env.
    pub targets: Vec<TransformTarget>,
    /// Controls TypeScript lowering.
    pub typescript: Option<TypeScriptOptions>,
    /// Controls JSX lowering.
    pub jsx: Option<JsxOptions>,
    /// Helper loader configuration. The raw `oxc` base helper state is ignored.
    pub helper_loader: Option<HelperLoaderOptions>,
    /// Injected imports run after all transforms, before `define`.
    pub inject: Option<InjectOptions>,
    /// Replacements run after `inject` and all transforms.
    pub define: Option<DefineOptions>,
    /// Raw backend passthrough (base layer).
    pub oxc: Option<OxcTransformOptions>,
}

impl TransformOptions {
    /// Apply the compile context `cwd` onto resolved oxc options.
    ///
    /// The compile options `cwd` is the run-level source of truth. It always
    /// overwrites the oxc base layer's `cwd`.
    pub(crate) fn apply_cwd(
        base: &mut OxcTransformOptions,
        cwd: &Path,
    ) {
        base.cwd = cwd.to_path_buf();
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

    pub(crate) fn resolve(&self) -> OxcTransformOptions {
        let mut base: OxcTransformOptions =
            self.oxc.clone().unwrap_or_default();

        overlay_typescript(&mut base, self.typescript.as_ref());

        overlay_jsx(&mut base.jsx, self.jsx.as_ref());

        if matches!(effective_runtime(self.jsx.as_ref()), JsxRuntime::Classic) {
            if let Some(pragma) = &base.jsx.pragma {
                base.typescript.jsx_pragma = pragma.clone().into();
            }

            if let Some(pragma_frag) = &base.jsx.pragma_frag {
                base.typescript.jsx_pragma_frag = pragma_frag.clone().into();
            }
        }

        if let Some(engine_targets) = Self::engine_targets(&self.targets) {
            base.env = oxc::transformer::EnvOptions::from(engine_targets);
        }

        overlay_helper_loader(
            &mut base.helper_loader,
            self.helper_loader.as_ref(),
        );

        base
    }

    /// Reject option combinations that cannot produce the requested output.
    pub(crate) fn validate(
        &self,
        resolved: &OxcTransformOptions,
    ) -> Result<(), String> {
        if self
            .typescript
            .as_ref()
            .and_then(|typescript| typescript.emit_decorator_metadata)
            == Some(true)
            && !resolved.decorator.legacy
        {
            return Err(String::from(
                "typescript.emitDecoratorMetadata requires typescript.experimentalDecorators",
            ));
        }

        Ok(())
    }

    /// Derive the output file name for `options` when `output.file` is unset
    /// or empty.
    ///
    /// Returns `None` in three cases. The caller set a non-empty
    /// `output.file`. The effective grammar has no output mapping. The derived
    /// name would equal the input file.
    pub(crate) fn output_file(
        &self,
        options: &ResolvableOptions,
    ) -> Option<String> {
        let explicit_output: bool = options
            .output
            .as_ref()
            .and_then(|output| output.file.as_deref())
            .is_some_and(|file| !file.is_empty());

        if explicit_output {
            return None;
        }

        let file: &str = options
            .file
            .as_deref()
            .filter(|file: &&str| !file.is_empty())
            .unwrap_or(FILE_DEFAULT);

        let language: Language = infer_language(file, options.language);

        if matches!(language, Language::JS | Language::DTS) {
            return None;
        }

        let jsx_preserved: bool = matches!(
            effective_runtime(self.jsx.as_ref()),
            JsxRuntime::Preserve
        );

        derive_output_file(file, language, jsx_preserved)
    }

    /// Resolve the helper loader state from the telarel layer.
    ///
    /// The inline helpers pass consumes this state. [`TransformOptions::resolve`]
    /// applies the same mapping to the oxc options. The raw `oxc` base helper
    /// state is ignored.
    pub(crate) fn resolve_helpers(&self) -> ResolvedHelpers {
        resolve_helpers(self.helper_loader.as_ref())
    }

    /// Resolve the `inject` options into the oxc configuration.
    ///
    /// Returns `None` when no entries are configured.
    pub(crate) fn resolve_inject(&self) -> Option<InjectGlobalVariablesConfig> {
        match &self.inject {
            | Some(inject) if !inject.entries.is_empty() => {
                Some(inject.resolve())
            },
            | _ => None,
        }
    }

    /// Resolve the `define` options into the oxc configuration.
    ///
    /// Returns `None` when no entries are configured. Errors from invalid keys
    /// or values surface as [`Diagnostics`].
    pub(crate) fn resolve_define(
        &self
    ) -> Result<Option<ReplaceGlobalDefinesConfig>, Diagnostics> {
        match &self.define {
            | Some(define) if !define.entries.is_empty() => {
                define.resolve().map(Some)
            },
            | _ => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use oxc::transformer::CompilerAssumptions;
    use oxc::transformer::DecoratorOptions;
    use oxc::transformer::Engine;
    use oxc::transformer::EngineTargets;
    use oxc::transformer::JsxOptions as OxcJsxOptions;
    use oxc::transformer::TransformOptions as OxcTransformOptions;
    use oxc_compat::Version;
    use telarel_common::OutputOptions;

    use crate::options::define::DefineOptions;
    use crate::options::helper_loader::{
        DEFAULT_HELPER_MODULE_NAME, HelperLoaderMode, HelperLoaderOptions,
    };
    use crate::options::inject::{InjectEntry, InjectOptions, InjectSpecifier};
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

        TransformOptions::apply_cwd(&mut base, Path::new("/repo"));

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
        assert!(options.define.is_none());
        assert!(options.inject.is_none());
        assert!(options.helper_loader.is_none());
        assert!(options.oxc.is_none());
    }

    #[test]
    fn test_resolve_inject_none_by_default() {
        let options: TransformOptions = TransformOptions::default();

        assert!(options.resolve_inject().is_none());
    }

    #[test]
    fn test_resolve_inject_empty_entries_is_none() {
        let options: TransformOptions = TransformOptions {
            inject: Some(InjectOptions::default()),
            ..TransformOptions::default()
        };

        assert!(options.resolve_inject().is_none());
    }

    #[test]
    fn test_resolve_inject_configured() {
        let options: TransformOptions = TransformOptions {
            inject: Some(InjectOptions {
                entries: vec![InjectEntry {
                    source: String::from("jquery"),
                    specifier: InjectSpecifier::Named {
                        imported: None,
                        local: String::from("$"),
                    },
                }],
            }),
            ..TransformOptions::default()
        };

        assert!(options.resolve_inject().is_some());
    }

    #[test]
    fn test_resolve_define_none_by_default() {
        let options: TransformOptions = TransformOptions::default();

        let resolved: Option<ReplaceGlobalDefinesConfig> =
            options.resolve_define().expect("default resolves");

        assert!(resolved.is_none());
    }

    #[test]
    fn test_resolve_define_empty_entries_is_none() {
        let options: TransformOptions = TransformOptions {
            define: Some(DefineOptions::default()),
            ..TransformOptions::default()
        };

        let resolved: Option<ReplaceGlobalDefinesConfig> =
            options.resolve_define().expect("empty resolves");

        assert!(resolved.is_none());
    }

    #[test]
    fn test_resolve_define_configured() {
        let options: TransformOptions = TransformOptions {
            define: Some(DefineOptions {
                entries: vec![(String::from("__DEV__"), String::from("false"))],
            }),
            ..TransformOptions::default()
        };

        let resolved: Option<ReplaceGlobalDefinesConfig> =
            options.resolve_define().expect("configured resolves");

        assert!(resolved.is_some());
    }

    #[test]
    fn test_resolve_define_invalid_entry_errors() {
        let options: TransformOptions = TransformOptions {
            define: Some(DefineOptions {
                entries: vec![(String::from("bad key"), String::from("1"))],
            }),
            ..TransformOptions::default()
        };

        assert!(options.resolve_define().is_err());
    }

    #[test]
    fn test_resolve_defaults() {
        let options: TransformOptions = TransformOptions::default();

        let resolved: OxcTransformOptions = options.resolve();

        assert!(resolved.env.es2015.arrow_function.is_none());
        assert!(!resolved.jsx.jsx_plugin);
        assert!(!resolved.jsx.development);
        assert!(resolved.jsx.refresh.is_none());
        assert!(!resolved.assumptions.set_public_class_fields);
        assert!(!resolved.typescript.remove_class_fields_without_initializer);
        assert!(resolved.typescript.allow_declare_fields);
    }

    #[test]
    fn test_resolve_class_fields_keeps_oxc_when_curated_unset() {
        let options: TransformOptions = TransformOptions {
            oxc: Some(OxcTransformOptions {
                assumptions: CompilerAssumptions {
                    set_public_class_fields: true,
                    ..CompilerAssumptions::default()
                },
                typescript: oxc::transformer::TypeScriptOptions {
                    allow_declare_fields: false,
                    ..oxc::transformer::TypeScriptOptions::default()
                },
                ..OxcTransformOptions::default()
            }),
            ..TransformOptions::default()
        };

        let resolved: OxcTransformOptions = options.resolve();

        assert!(resolved.assumptions.set_public_class_fields);
        assert!(!resolved.typescript.remove_class_fields_without_initializer);
        assert!(!resolved.typescript.allow_declare_fields);
    }

    #[test]
    fn test_resolve_class_fields_explicit_wins_over_oxc() {
        let options: TransformOptions = TransformOptions {
            typescript: Some(TypeScriptOptions {
                use_define_for_class_fields: Some(true),
                ..TypeScriptOptions::default()
            }),
            oxc: Some(OxcTransformOptions {
                assumptions: CompilerAssumptions {
                    set_public_class_fields: true,
                    ..CompilerAssumptions::default()
                },
                typescript: oxc::transformer::TypeScriptOptions {
                    remove_class_fields_without_initializer: true,
                    ..oxc::transformer::TypeScriptOptions::default()
                },
                ..OxcTransformOptions::default()
            }),
            ..TransformOptions::default()
        };

        let resolved: OxcTransformOptions = options.resolve();

        assert!(!resolved.assumptions.set_public_class_fields);
        assert!(!resolved.typescript.remove_class_fields_without_initializer);
        assert!(resolved.typescript.allow_declare_fields);
    }

    #[test]
    fn test_resolve_preserves_raw_oxc_jsx_passes_by_default() {
        let options: TransformOptions = TransformOptions {
            oxc: Some(OxcTransformOptions {
                jsx: OxcJsxOptions {
                    jsx_self_plugin: true,
                    jsx_source_plugin: true,
                    development: true,
                    refresh: Some(
                        oxc::transformer::ReactRefreshOptions::default(),
                    ),
                    ..OxcJsxOptions::default()
                },
                ..OxcTransformOptions::default()
            }),
            ..TransformOptions::default()
        };

        let resolved: OxcTransformOptions = options.resolve();

        assert!(!resolved.jsx.jsx_plugin);
        assert!(!resolved.jsx.display_name_plugin);
        assert!(!resolved.jsx.jsx_self_plugin);
        assert!(!resolved.jsx.jsx_source_plugin);
        assert!(!resolved.jsx.development);
        assert!(resolved.jsx.refresh.is_none());
    }

    #[test]
    fn test_resolve_jsx_runtime_none_preserves() {
        let options: TransformOptions = TransformOptions {
            jsx: Some(JsxOptions {
                runtime: None,
                development: Some(true),
                refresh: Some(true),
                ..JsxOptions::default()
            }),
            ..TransformOptions::default()
        };

        let resolved: OxcTransformOptions = options.resolve();

        assert!(!resolved.jsx.jsx_plugin);
        assert!(!resolved.jsx.development);
        assert!(resolved.jsx.refresh.is_none());
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
    fn test_resolve_typescript_overlay() {
        let options: TransformOptions = TransformOptions {
            typescript: Some(TypeScriptOptions {
                verbatim_module_syntax: Some(true),
                optimize_const_enums: Some(true),
                ..TypeScriptOptions::default()
            }),
            oxc: Some(OxcTransformOptions {
                typescript: oxc::transformer::TypeScriptOptions {
                    only_remove_type_imports: false,
                    ..oxc::transformer::TypeScriptOptions::default()
                },
                ..OxcTransformOptions::default()
            }),
            ..TransformOptions::default()
        };

        let resolved: OxcTransformOptions = options.resolve();

        assert!(resolved.typescript.only_remove_type_imports);
        assert!(resolved.typescript.optimize_const_enums);
    }

    #[test]
    fn test_resolve_typescript_classic_pragmas_sync_to_typescript() {
        let options: TransformOptions = TransformOptions {
            jsx: Some(JsxOptions {
                runtime: Some(JsxRuntime::Classic),
                pragma: Some(String::from("h")),
                pragma_frag: Some(String::from("Fragment")),
                ..JsxOptions::default()
            }),
            ..TransformOptions::default()
        };

        let resolved: OxcTransformOptions = options.resolve();

        assert_eq!(resolved.jsx.pragma.as_deref(), Some("h"));
        assert_eq!(resolved.typescript.jsx_pragma, "h");
        assert_eq!(resolved.typescript.jsx_pragma_frag, "Fragment");
    }

    #[test]
    fn test_resolve_typescript_raw_classic_pragmas_sync_to_typescript() {
        let options: TransformOptions = TransformOptions {
            jsx: Some(JsxOptions {
                runtime: Some(JsxRuntime::Classic),
                ..JsxOptions::default()
            }),
            oxc: Some(OxcTransformOptions {
                jsx: OxcJsxOptions {
                    pragma: Some(String::from("h")),
                    pragma_frag: Some(String::from("Fragment")),
                    ..OxcJsxOptions::default()
                },
                ..OxcTransformOptions::default()
            }),
            ..TransformOptions::default()
        };

        let resolved: OxcTransformOptions = options.resolve();

        assert_eq!(resolved.typescript.jsx_pragma, "h");
        assert_eq!(resolved.typescript.jsx_pragma_frag, "Fragment");
    }

    #[test]
    fn test_resolve_typescript_non_classic_pragmas_do_not_sync() {
        for runtime in [JsxRuntime::Preserve, JsxRuntime::Automatic] {
            let options: TransformOptions = TransformOptions {
                jsx: Some(JsxOptions {
                    runtime: Some(runtime),
                    pragma: Some(String::from("h")),
                    pragma_frag: Some(String::from("Fragment")),
                    ..JsxOptions::default()
                }),
                ..TransformOptions::default()
            };

            let resolved: OxcTransformOptions = options.resolve();

            assert_eq!(
                resolved.typescript.jsx_pragma, "React.createElement",
                "{runtime:?}"
            );
            assert_eq!(
                resolved.typescript.jsx_pragma_frag, "React.Fragment",
                "{runtime:?}"
            );
        }
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
    fn test_oxc_base_survives_clone() {
        let options: TransformOptions = TransformOptions {
            oxc: Some(oxc_base()),
            ..TransformOptions::default()
        };

        let resolved_oxc: OxcTransformOptions = options.oxc.expect("oxc layer");

        assert_eq!(resolved_oxc.jsx.pragma.as_deref(), Some("h"));
    }

    #[test]
    fn test_resolve_oxc_helper_loader_base_is_overwritten() {
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
            oxc::transformer::HelperLoaderMode::Runtime
        ));
        assert_eq!(
            resolved.helper_loader.module_name,
            DEFAULT_HELPER_MODULE_NAME
        );
    }

    #[test]
    fn test_resolve_helper_absent_defaults_to_inline() {
        let options: TransformOptions = TransformOptions::default();

        let resolved: OxcTransformOptions = options.resolve();
        let helpers: ResolvedHelpers = options.resolve_helpers();

        assert!(matches!(
            resolved.helper_loader.mode,
            oxc::transformer::HelperLoaderMode::Runtime
        ));
        assert!(helpers.inline());
        assert_eq!(helpers.module_name, DEFAULT_HELPER_MODULE_NAME);
    }

    #[test]
    fn test_resolve_helper_inline_maps_to_oxc_runtime_with_inline_flag() {
        let options: TransformOptions = TransformOptions {
            helper_loader: Some(HelperLoaderOptions {
                mode: Some(HelperLoaderMode::Inline),
                module_name: None,
            }),
            ..TransformOptions::default()
        };

        let resolved: OxcTransformOptions = options.resolve();
        let helpers: ResolvedHelpers = options.resolve_helpers();

        assert!(matches!(
            resolved.helper_loader.mode,
            oxc::transformer::HelperLoaderMode::Runtime
        ));
        assert!(helpers.inline());
    }

    #[test]
    fn test_resolve_helper_inline_keeps_module_name() {
        let options: TransformOptions = TransformOptions {
            helper_loader: Some(HelperLoaderOptions {
                mode: Some(HelperLoaderMode::Inline),
                module_name: Some(String::from("my-runtime")),
            }),
            ..TransformOptions::default()
        };

        let resolved: OxcTransformOptions = options.resolve();
        let helpers: ResolvedHelpers = options.resolve_helpers();

        assert_eq!(resolved.helper_loader.module_name, "my-runtime");
        assert!(helpers.inline());
        assert_eq!(helpers.module_name, "my-runtime");
    }

    #[test]
    fn test_resolve_helper_runtime_maps_to_oxc_runtime() {
        let options: TransformOptions = TransformOptions {
            helper_loader: Some(HelperLoaderOptions {
                mode: Some(HelperLoaderMode::Runtime),
                module_name: None,
            }),
            ..TransformOptions::default()
        };

        let resolved: OxcTransformOptions = options.resolve();
        let helpers: ResolvedHelpers = options.resolve_helpers();

        assert!(matches!(
            resolved.helper_loader.mode,
            oxc::transformer::HelperLoaderMode::Runtime
        ));
        assert!(!helpers.inline());
    }

    #[test]
    fn test_resolve_helper_external_maps_to_oxc_external() {
        let options: TransformOptions = TransformOptions {
            helper_loader: Some(HelperLoaderOptions {
                mode: Some(HelperLoaderMode::External),
                module_name: None,
            }),
            ..TransformOptions::default()
        };

        let resolved: OxcTransformOptions = options.resolve();
        let helpers: ResolvedHelpers = options.resolve_helpers();

        assert!(matches!(
            resolved.helper_loader.mode,
            oxc::transformer::HelperLoaderMode::External
        ));
        assert!(!helpers.inline());
    }

    #[test]
    fn test_resolve_helper_absent_mode_defaults_to_inline() {
        let options: TransformOptions = TransformOptions {
            helper_loader: Some(HelperLoaderOptions {
                mode: None,
                module_name: None,
            }),
            ..TransformOptions::default()
        };

        let resolved: OxcTransformOptions = options.resolve();
        let helpers: ResolvedHelpers = options.resolve_helpers();

        assert!(matches!(
            resolved.helper_loader.mode,
            oxc::transformer::HelperLoaderMode::Runtime
        ));
        assert!(helpers.inline());
        assert_eq!(helpers.module_name, DEFAULT_HELPER_MODULE_NAME);
    }

    #[test]
    fn test_resolve_helper_wins_over_oxc_base() {
        let options: TransformOptions = TransformOptions {
            helper_loader: Some(HelperLoaderOptions {
                mode: Some(HelperLoaderMode::External),
                module_name: Some(String::from("my-runtime")),
            }),
            oxc: Some(OxcTransformOptions {
                helper_loader: oxc::transformer::HelperLoaderOptions {
                    mode: oxc::transformer::HelperLoaderMode::Runtime,
                    module_name: std::borrow::Cow::Borrowed("other-runtime"),
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
        assert_eq!(resolved.helper_loader.module_name, "my-runtime");
    }

    #[test]
    fn test_resolve_oxc_base_helper_loader_is_ignored() {
        let options: TransformOptions = TransformOptions {
            oxc: Some(OxcTransformOptions {
                helper_loader: oxc::transformer::HelperLoaderOptions {
                    mode: oxc::transformer::HelperLoaderMode::Inline,
                    module_name: std::borrow::Cow::Borrowed("my-runtime"),
                },
                ..OxcTransformOptions::default()
            }),
            ..TransformOptions::default()
        };

        let resolved: OxcTransformOptions = options.resolve();
        let helpers: ResolvedHelpers = options.resolve_helpers();

        assert!(matches!(
            resolved.helper_loader.mode,
            oxc::transformer::HelperLoaderMode::Runtime
        ));
        assert_eq!(
            resolved.helper_loader.module_name,
            DEFAULT_HELPER_MODULE_NAME
        );
        assert!(helpers.inline());
        assert_eq!(helpers.module_name, DEFAULT_HELPER_MODULE_NAME);
    }

    #[test]
    fn test_validate_emit_decorator_metadata_requires_legacy() {
        let options: TransformOptions = TransformOptions {
            typescript: Some(TypeScriptOptions {
                emit_decorator_metadata: Some(true),
                ..TypeScriptOptions::default()
            }),
            ..TransformOptions::default()
        };

        let resolved: OxcTransformOptions = options.resolve();

        assert!(options.validate(&resolved).is_err());
    }

    #[test]
    fn test_validate_emit_decorator_metadata_with_curated_legacy() {
        let options: TransformOptions = TransformOptions {
            typescript: Some(TypeScriptOptions {
                emit_decorator_metadata: Some(true),
                experimental_decorators: Some(true),
                ..TypeScriptOptions::default()
            }),
            ..TransformOptions::default()
        };

        let resolved: OxcTransformOptions = options.resolve();

        assert!(options.validate(&resolved).is_ok());
    }

    #[test]
    fn test_validate_emit_decorator_metadata_with_raw_legacy() {
        let options: TransformOptions = TransformOptions {
            typescript: Some(TypeScriptOptions {
                emit_decorator_metadata: Some(true),
                ..TypeScriptOptions::default()
            }),
            oxc: Some(OxcTransformOptions {
                decorator: DecoratorOptions {
                    legacy: true,
                    ..DecoratorOptions::default()
                },
                ..OxcTransformOptions::default()
            }),
            ..TransformOptions::default()
        };

        let resolved: OxcTransformOptions = options.resolve();

        assert!(options.validate(&resolved).is_ok());
    }

    fn resolvable(
        file: Option<&str>,
        language: Option<Language>,
    ) -> ResolvableOptions {
        ResolvableOptions {
            file: file.map(String::from),
            language,
            ..ResolvableOptions::default()
        }
    }

    fn assert_output_file(
        options: &TransformOptions,
        resolvable: &ResolvableOptions,
        expected: Option<&str>,
    ) {
        assert_eq!(options.output_file(resolvable).as_deref(), expected);
    }

    #[test]
    fn test_output_file_derives_typescript_suffixes() {
        let options: TransformOptions = TransformOptions::default();

        assert_output_file(
            &options,
            &resolvable(Some("src/app.ts"), None),
            Some("src/app.js"),
        );
        assert_output_file(
            &options,
            &resolvable(Some("src/app.mts"), None),
            Some("src/app.mjs"),
        );
        assert_output_file(
            &options,
            &resolvable(Some("src/app.cts"), None),
            Some("src/app.cjs"),
        );
    }

    #[test]
    fn test_output_file_keeps_typescript_module_flavors() {
        let options: TransformOptions = TransformOptions::default();

        assert_output_file(
            &options,
            &resolvable(Some("src/app.mjs"), Some(Language::TS)),
            None,
        );
        assert_output_file(
            &options,
            &resolvable(Some("src/app.cjs"), Some(Language::TS)),
            None,
        );
    }

    #[test]
    fn test_output_file_uses_default_for_missing_and_empty_file() {
        let options: TransformOptions = TransformOptions::default();

        assert_output_file(&options, &resolvable(None, None), Some("index.js"));
        assert_output_file(
            &options,
            &resolvable(Some(""), None),
            Some("index.js"),
        );
    }

    #[test]
    fn test_output_file_explicit_output_wins() {
        let options: TransformOptions = TransformOptions::default();
        let resolvable: ResolvableOptions = ResolvableOptions {
            file: Some(String::from("src/app.ts")),
            output: Some(OutputOptions {
                file: Some(String::from("dist/custom.js")),
            }),
            ..ResolvableOptions::default()
        };

        assert_output_file(&options, &resolvable, None);
    }

    #[test]
    fn test_output_file_fills_empty_output() {
        let options: TransformOptions = TransformOptions::default();
        let resolvable: ResolvableOptions = ResolvableOptions {
            file: Some(String::from("src/app.ts")),
            output: Some(OutputOptions { file: Some(String::new()) }),
            ..ResolvableOptions::default()
        };

        assert_output_file(&options, &resolvable, Some("src/app.js"));
    }

    #[test]
    fn test_output_file_javascript_and_declarations_are_unchanged() {
        let options: TransformOptions = TransformOptions::default();

        assert_output_file(
            &options,
            &resolvable(Some("src/app.js"), None),
            None,
        );
        assert_output_file(
            &options,
            &resolvable(Some("src/app.d.ts"), None),
            None,
        );
    }

    #[test]
    fn test_output_file_tsx_preserved_maps_to_jsx() {
        let options: TransformOptions = TransformOptions::default();

        assert_output_file(
            &options,
            &resolvable(Some("src/app.tsx"), None),
            Some("src/app.jsx"),
        );
    }

    #[test]
    fn test_output_file_tsx_lowered_maps_to_js() {
        for runtime in [JsxRuntime::Classic, JsxRuntime::Automatic] {
            let options: TransformOptions = TransformOptions {
                jsx: Some(JsxOptions {
                    runtime: Some(runtime),
                    ..JsxOptions::default()
                }),
                ..TransformOptions::default()
            };

            assert_output_file(
                &options,
                &resolvable(Some("src/app.tsx"), None),
                Some("src/app.js"),
            );
        }
    }

    #[test]
    fn test_output_file_jsx_preserved_is_unchanged() {
        let options: TransformOptions = TransformOptions::default();

        assert_output_file(
            &options,
            &resolvable(Some("src/app.jsx"), None),
            None,
        );
    }

    #[test]
    fn test_output_file_jsx_lowered_maps_to_js() {
        let options: TransformOptions = TransformOptions {
            jsx: Some(JsxOptions {
                runtime: Some(JsxRuntime::Automatic),
                ..JsxOptions::default()
            }),
            ..TransformOptions::default()
        };

        assert_output_file(
            &options,
            &resolvable(Some("src/app.jsx"), None),
            Some("src/app.js"),
        );
    }

    #[test]
    fn test_output_file_strips_query_and_hash() {
        let options: TransformOptions = TransformOptions::default();

        assert_output_file(
            &options,
            &resolvable(Some("src/app.ts?v=1#hash"), None),
            Some("src/app.js"),
        );
        assert_output_file(
            &options,
            &resolvable(Some("src/app.tsx?v=1"), None),
            Some("src/app.jsx"),
        );
    }

    #[test]
    fn test_output_file_extensionless_and_unknown() {
        let options: TransformOptions = TransformOptions::default();

        assert_output_file(
            &options,
            &resolvable(Some("src/app"), Some(Language::TS)),
            Some("src/app.js"),
        );
        assert_output_file(
            &options,
            &resolvable(Some("src/app"), Some(Language::TSX)),
            Some("src/app.jsx"),
        );
        assert_output_file(
            &options,
            &resolvable(Some("src/app.css"), Some(Language::TS)),
            None,
        );
        assert_output_file(
            &options,
            &resolvable(Some("src/app.css"), Some(Language::TSX)),
            None,
        );
    }

    #[test]
    fn test_output_file_explicit_language_wins() {
        let options: TransformOptions = TransformOptions::default();

        assert_output_file(
            &options,
            &resolvable(Some("src/app.js"), Some(Language::TSX)),
            Some("src/app.jsx"),
        );
        assert_output_file(
            &options,
            &resolvable(Some("src/app.ts"), Some(Language::JS)),
            None,
        );
    }

    #[test]
    fn test_output_file_raw_oxc_jsx_does_not_lower() {
        let options: TransformOptions = TransformOptions {
            oxc: Some(OxcTransformOptions {
                jsx: OxcJsxOptions {
                    runtime: oxc::transformer::JsxRuntime::Automatic,
                    jsx_plugin: true,
                    ..OxcJsxOptions::default()
                },
                ..OxcTransformOptions::default()
            }),
            ..TransformOptions::default()
        };

        assert_output_file(
            &options,
            &resolvable(Some("src/app.tsx"), None),
            Some("src/app.jsx"),
        );
    }

    #[test]
    fn test_output_file_preserve_wins_over_development_and_refresh() {
        let options: TransformOptions = TransformOptions {
            jsx: Some(JsxOptions {
                runtime: Some(JsxRuntime::Preserve),
                development: Some(true),
                refresh: Some(true),
                ..JsxOptions::default()
            }),
            ..TransformOptions::default()
        };

        assert_output_file(
            &options,
            &resolvable(Some("src/app.tsx"), None),
            Some("src/app.jsx"),
        );
    }
}
