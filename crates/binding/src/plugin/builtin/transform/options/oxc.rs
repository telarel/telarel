use telarel_plugin_transform::HelperLoaderMode as TelarelHelperLoaderMode;
use telarel_plugin_transform::HelperLoaderOptions as TelarelHelperLoaderOptions;

/// The JS-facing helper loader mode.
#[derive(Debug, Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BindingHelperLoaderMode {
    /// Helper functions are directly inserted into the program.
    Inline,
    /// Helper functions are accessed from a global `babelHelpers` object.
    External,
    /// Helper functions are imported from the runtime package.
    Runtime,
}

impl From<BindingHelperLoaderMode> for TelarelHelperLoaderMode {
    fn from(mode: BindingHelperLoaderMode) -> Self {
        match mode {
            | BindingHelperLoaderMode::Inline => Self::Inline,
            | BindingHelperLoaderMode::External => Self::External,
            | BindingHelperLoaderMode::Runtime => Self::Runtime,
        }
    }
}

/// The JS-facing helper loader options.
///
/// Mirrors oxc's `HelperLoaderOptions` with camelCase keys: oxc's own struct
/// has no `rename_all`, so its JSON key would be `module_name`.
#[derive(Debug, Default, serde::Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct BindingHelperLoaderOptions {
    /// The module name to import helper functions from.
    module_name: Option<String>,
    /// Strategy used to resolve helper calls.
    mode: Option<BindingHelperLoaderMode>,
}

impl BindingHelperLoaderOptions {
    /// Convert into the telarel-owned helper loader options.
    fn into_helper_loader_options(self) -> TelarelHelperLoaderOptions {
        TelarelHelperLoaderOptions {
            module_name: self.module_name,
            mode: self.mode.map(From::from),
        }
    }
}

/// A deserializable subset of oxc's transform options for the raw `oxc` passthrough layer.
#[derive(Debug, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OxcPassthrough {
    #[serde(default)]
    assumptions: Option<oxc::transformer::CompilerAssumptions>,
    #[serde(default)]
    typescript: Option<oxc::transformer::TypeScriptOptions>,
    #[serde(default)]
    decorator: Option<oxc::transformer::DecoratorOptions>,
    #[serde(default)]
    jsx: Option<oxc::transformer::JsxOptions>,
    #[serde(default)]
    env: Option<oxc::transformer::EnvOptions>,
    #[serde(default)]
    helper_loader: Option<BindingHelperLoaderOptions>,
}

impl OxcPassthrough {
    /// Convert into the raw oxc base layer plus the telarel helper options.
    pub fn into_parts(
        self
    ) -> (oxc::transformer::TransformOptions, Option<TelarelHelperLoaderOptions>)
    {
        let helper_loader: Option<TelarelHelperLoaderOptions> = self
            .helper_loader
            .map(BindingHelperLoaderOptions::into_helper_loader_options);

        let oxc: oxc::transformer::TransformOptions =
            oxc::transformer::TransformOptions {
                assumptions: self.assumptions.unwrap_or_default(),
                typescript: self.typescript.unwrap_or_default(),
                decorator: self.decorator.unwrap_or_default(),
                jsx: self.jsx.unwrap_or_default(),
                env: self.env.unwrap_or_default(),
                helper_loader: oxc::transformer::HelperLoaderOptions::default(),
                ..oxc::transformer::TransformOptions::default()
            };

        (oxc, helper_loader)
    }
}

/// Convert the optional passthrough layer into the oxc base and helper options.
///
/// Defined here because the `oxc` module name shadows the oxc crate inside the parent `options` module.
pub fn to_oxc_parts(
    passthrough: Option<OxcPassthrough>
) -> (
    Option<oxc::transformer::TransformOptions>,
    Option<TelarelHelperLoaderOptions>,
) {
    match passthrough {
        | Some(passthrough) => {
            let (oxc, helper_loader): (
                oxc::transformer::TransformOptions,
                Option<TelarelHelperLoaderOptions>,
            ) = passthrough.into_parts();

            (Some(oxc), helper_loader)
        },
        | None => (None, None),
    }
}

#[cfg(test)]
mod tests {
    use telarel_plugin_transform::HelperLoaderMode as TelarelHelperLoaderMode;

    use super::OxcPassthrough;

    #[test]
    fn test_oxc_passthrough_defaults_fill_oxc_defaults() {
        let passthrough: OxcPassthrough = OxcPassthrough::default();

        let (oxc, helper): (
            oxc::transformer::TransformOptions,
            Option<telarel_plugin_transform::HelperLoaderOptions>,
        ) = passthrough.into_parts();

        assert!(oxc.jsx.jsx_plugin);
        assert_eq!(oxc.jsx.runtime, oxc::transformer::JsxRuntime::Automatic);
        assert!(matches!(
            oxc.helper_loader.mode,
            oxc::transformer::HelperLoaderMode::Runtime
        ));
        assert!(helper.is_none());
    }

    #[test]
    fn test_oxc_passthrough_helper_loader_defaults() {
        let passthrough: OxcPassthrough =
            serde_json::from_str("{}").expect("empty object parses");

        let (_, helper): (
            oxc::transformer::TransformOptions,
            Option<telarel_plugin_transform::HelperLoaderOptions>,
        ) = passthrough.into_parts();

        // An absent `helperLoader` stays `None`; telarel's unconfigured
        // default is then inline, handled by its own post-transform pass.
        assert!(helper.is_none());
    }

    #[test]
    fn test_oxc_passthrough_helper_loader_fields() {
        let passthrough: OxcPassthrough = serde_json::from_str(
            r#"{ "helperLoader": { "mode": "external", "moduleName": "my-runtime" } }"#,
        )
        .expect("helper loader parses");

        let (_, helper): (
            oxc::transformer::TransformOptions,
            Option<telarel_plugin_transform::HelperLoaderOptions>,
        ) = passthrough.into_parts();

        let helper: telarel_plugin_transform::HelperLoaderOptions =
            helper.expect("helper loader layer");

        assert_eq!(helper.module_name.as_deref(), Some("my-runtime"));
        assert_eq!(helper.mode, Some(TelarelHelperLoaderMode::External));
    }

    #[test]
    fn test_oxc_passthrough_helper_loader_partial() {
        let passthrough: OxcPassthrough = serde_json::from_str(
            r#"{ "helperLoader": { "mode": "external" } }"#,
        )
        .expect("partial helper loader parses");

        let (_, helper): (
            oxc::transformer::TransformOptions,
            Option<telarel_plugin_transform::HelperLoaderOptions>,
        ) = passthrough.into_parts();

        let helper: telarel_plugin_transform::HelperLoaderOptions =
            helper.expect("helper loader layer");

        assert!(helper.module_name.is_none());
        assert_eq!(helper.mode, Some(TelarelHelperLoaderMode::External));
    }

    #[test]
    fn test_oxc_passthrough_helper_loader_inline_maps_to_telarel_inline() {
        let passthrough: OxcPassthrough =
            serde_json::from_str(r#"{ "helperLoader": { "mode": "inline" } }"#)
                .expect("inline helper loader parses");

        let (oxc, helper): (
            oxc::transformer::TransformOptions,
            Option<telarel_plugin_transform::HelperLoaderOptions>,
        ) = passthrough.into_parts();

        assert!(matches!(
            oxc.helper_loader.mode,
            oxc::transformer::HelperLoaderMode::Runtime
        ));

        let helper: telarel_plugin_transform::HelperLoaderOptions =
            helper.expect("helper loader layer");

        assert_eq!(helper.mode, Some(TelarelHelperLoaderMode::Inline));
    }

    #[test]
    fn test_oxc_passthrough_helper_loader_unknown_field_errors() {
        let result: Result<OxcPassthrough, _> =
            serde_json::from_str(r#"{ "helperLoader": { "helper": true } }"#);

        assert!(result.is_err());
    }
}
