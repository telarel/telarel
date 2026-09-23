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

impl From<BindingHelperLoaderMode> for oxc::transformer::HelperLoaderMode {
    fn from(mode: BindingHelperLoaderMode) -> Self {
        match mode {
            BindingHelperLoaderMode::Inline => Self::Inline,
            BindingHelperLoaderMode::External => Self::External,
            BindingHelperLoaderMode::Runtime => Self::Runtime,
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
    /// Convert into the raw oxc helper loader options;
    /// absent fields fall back to oxc's own defaults.
    fn into_helper_loader_options(
        self
    ) -> oxc::transformer::HelperLoaderOptions {
        oxc::transformer::HelperLoaderOptions {
            module_name: self
                .module_name
                .map_or_else(default_module_name, From::from)
                .into(),
            mode: self
                .mode
                .map_or(oxc::transformer::HelperLoaderMode::Runtime, From::from),
        }
    }
}

/// The default helper module name, matching oxc's own default.
fn default_module_name() -> String {
    String::from("@oxc-project/runtime")
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
    /// Convert into the raw oxc base layer;
    /// absent sub-options fall back to oxc's own defaults.
    pub fn into_transform_options(self) -> oxc::transformer::TransformOptions {
        oxc::transformer::TransformOptions {
            assumptions: self.assumptions.unwrap_or_default(),
            typescript: self.typescript.unwrap_or_default(),
            decorator: self.decorator.unwrap_or_default(),
            jsx: self.jsx.unwrap_or_default(),
            env: self.env.unwrap_or_default(),
            helper_loader: self.helper_loader.map_or_else(
                oxc::transformer::HelperLoaderOptions::default,
                |helper_loader| helper_loader.into_helper_loader_options(),
            ),
            ..oxc::transformer::TransformOptions::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use oxc::transformer::HelperLoaderMode;

    use super::OxcPassthrough;

    #[test]
    fn test_oxc_passthrough_defaults_fill_oxc_defaults() {
        let passthrough: OxcPassthrough = OxcPassthrough::default();

        let oxc: oxc::transformer::TransformOptions =
            passthrough.into_transform_options();

        assert!(oxc.jsx.jsx_plugin);
        assert_eq!(oxc.jsx.runtime, oxc::transformer::JsxRuntime::Automatic);
        assert_eq!(oxc.helper_loader.module_name, "@oxc-project/runtime");
        assert!(matches!(
            oxc.helper_loader.mode,
            oxc::transformer::HelperLoaderMode::Runtime
        ));
    }

    #[test]
    fn test_oxc_passthrough_helper_loader_defaults() {
        let passthrough: OxcPassthrough =
            serde_json::from_str("{}").expect("empty object parses");

        let oxc: oxc::transformer::TransformOptions =
            passthrough.into_transform_options();

        assert_eq!(oxc.helper_loader.module_name, "@oxc-project/runtime");
        assert!(matches!(oxc.helper_loader.mode, HelperLoaderMode::Runtime));
    }

    #[test]
    fn test_oxc_passthrough_helper_loader_fields() {
        let passthrough: OxcPassthrough = serde_json::from_str(
            r#"{ "helperLoader": { "mode": "external", "moduleName": "my-runtime" } }"#,
        )
        .expect("helper loader parses");

        let oxc: oxc::transformer::TransformOptions =
            passthrough.into_transform_options();

        assert_eq!(oxc.helper_loader.module_name, "my-runtime");
        assert!(matches!(oxc.helper_loader.mode, HelperLoaderMode::External));
    }

    #[test]
    fn test_oxc_passthrough_helper_loader_partial() {
        let passthrough: OxcPassthrough = serde_json::from_str(
            r#"{ "helperLoader": { "mode": "external" } }"#,
        )
        .expect("partial helper loader parses");

        let oxc: oxc::transformer::TransformOptions =
            passthrough.into_transform_options();

        assert_eq!(oxc.helper_loader.module_name, "@oxc-project/runtime");
        assert!(matches!(oxc.helper_loader.mode, HelperLoaderMode::External));
    }

    #[test]
    fn test_oxc_passthrough_helper_loader_unknown_field_errors() {
        let result: Result<OxcPassthrough, _> =
            serde_json::from_str(r#"{ "helperLoader": { "helper": true } }"#);

        assert!(result.is_err());
    }
}
