use napi::{Error, Result, Status};

use telarel_plugin_transform::{
    JsxOptions as TelarelJsxOptions, TransformOptions, TransformTarget,
    TypeScriptOptions as TelarelTypeScriptOptions,
};

use crate::plugin::builtin::common::parse_optional_options;
use crate::plugin::builtin::transform::oxc::OxcPassthrough;
use crate::plugin::builtin::transform::target::parse_target;

/// The JS-facing options bag for the builtin transform plugin.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BindingTransformPluginOptions {
    pub targets: Option<Vec<serde_json::Value>>,
    pub jsx: Option<serde_json::Value>,
    pub typescript: Option<serde_json::Value>,
    pub oxc: Option<serde_json::Value>,
}

/// Deserialize the JS-facing options bag into the telarel transform options.
pub fn to_transform_options(
    options: BindingTransformPluginOptions
) -> Result<TransformOptions> {
    let mut parsed: Vec<TransformTarget> = Vec::new();

    if let Some(targets) = &options.targets {
        for target in targets {
            let parsed_target: TransformTarget =
                parse_target(target).map_err(|message: String| {
                    Error::new(Status::InvalidArg, message)
                })?;

            parsed.push(parsed_target);
        }
    }

    let jsx: Option<TelarelJsxOptions> =
        parse_optional_options(options.jsx, "jsx options")?;

    let typescript: Option<TelarelTypeScriptOptions> =
        parse_optional_options(options.typescript, "typescript options")?;

    let oxc: Option<OxcPassthrough> =
        parse_optional_options(options.oxc, "oxc options")?;

    Ok(TransformOptions {
        targets: parsed,
        jsx,
        typescript,
        oxc: oxc.map(OxcPassthrough::into_transform_options),
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use telarel_plugin_transform::{
        JsxRuntime, TransformOptions as TelarelTransformOptions,
        TransformTarget,
    };

    use super::{BindingTransformPluginOptions, to_transform_options};

    #[test]
    fn test_to_transform_options_defaults() {
        let options: BindingTransformPluginOptions =
            BindingTransformPluginOptions {
                targets: None,
                jsx: None,
                typescript: None,
                oxc: None,
            };

        let parsed: TelarelTransformOptions =
            to_transform_options(options).expect("defaults parse");

        assert!(parsed.targets.is_empty());
        assert!(parsed.jsx.is_none());
        assert!(parsed.typescript.is_none());
        assert!(parsed.oxc.is_none());
    }

    #[test]
    fn test_to_transform_options_targets() {
        let options: BindingTransformPluginOptions =
            BindingTransformPluginOptions {
                targets: Some(vec![
                    json!("es2022"),
                    json!({ "chrome": 100 }),
                    json!("ESNEXT"),
                ]),
                jsx: None,
                typescript: None,
                oxc: None,
            };

        let parsed: TelarelTransformOptions =
            to_transform_options(options).expect("targets parse");

        assert_eq!(
            parsed.targets,
            vec![
                TransformTarget::Es2022,
                TransformTarget::Chrome(100),
                TransformTarget::EsNext,
            ]
        );
    }

    #[test]
    fn test_to_transform_options_invalid_target_errors() {
        let options: BindingTransformPluginOptions =
            BindingTransformPluginOptions {
                targets: Some(vec![json!("chrome58")]),
                jsx: None,
                typescript: None,
                oxc: None,
            };

        let error: napi::Error =
            to_transform_options(options).expect_err("invalid target errors");

        assert_eq!(error.status, napi::Status::InvalidArg);
        assert!(error.reason.contains("invalid transform target"));
    }

    #[test]
    fn test_to_transform_options_jsx_and_typescript() {
        let options: BindingTransformPluginOptions =
            BindingTransformPluginOptions {
                targets: None,
                jsx: Some(json!({
                    "runtime": "classic",
                    "importSource": "preact",
                })),
                typescript: Some(json!({
                    "onlyRemoveTypeImports": true,
                    "jsxPragma": "h",
                })),
                oxc: None,
            };

        let parsed: TelarelTransformOptions =
            to_transform_options(options).expect("layers parse");

        let jsx: telarel_plugin_transform::JsxOptions =
            parsed.jsx.expect("jsx layer");

        assert_eq!(jsx.runtime, Some(JsxRuntime::Classic));
        assert_eq!(jsx.import_source.as_deref(), Some("preact"));

        let typescript: telarel_plugin_transform::TypeScriptOptions =
            parsed.typescript.expect("typescript layer");

        assert_eq!(typescript.only_remove_type_imports, Some(true));
        assert_eq!(typescript.jsx_pragma.as_deref(), Some("h"));
    }

    #[test]
    fn test_to_transform_options_jsx_unknown_field_errors() {
        let options: BindingTransformPluginOptions =
            BindingTransformPluginOptions {
                targets: None,
                jsx: Some(json!({ "unknownField": true })),
                typescript: None,
                oxc: None,
            };

        let error: napi::Error =
            to_transform_options(options).expect_err("unknown field errors");

        assert_eq!(error.status, napi::Status::InvalidArg);
        assert!(error.reason.contains("invalid `jsx options`"));
    }

    #[test]
    fn test_to_transform_options_oxc_passthrough() {
        let options: BindingTransformPluginOptions =
            BindingTransformPluginOptions {
                targets: None,
                jsx: None,
                typescript: None,
                oxc: Some(json!({
                    "jsx": { "pragma": "h" },
                    "env": { "targets": { "chrome": "80" } },
                })),
            };

        let parsed: TelarelTransformOptions =
            to_transform_options(options).expect("oxc layer parses");

        let oxc: oxc::transformer::TransformOptions =
            parsed.oxc.expect("oxc layer");

        assert_eq!(oxc.jsx.pragma.as_deref(), Some("h"));
        // Optional chaining landed in Chrome 91, so targeting Chrome 80
        // enables its lowering; class properties (Chrome 74+) are already
        // supported and their options stay unset.
        assert!(oxc.env.es2020.optional_chaining);
    }

    #[test]
    fn test_to_transform_options_oxc_unknown_field_errors() {
        let options: BindingTransformPluginOptions =
            BindingTransformPluginOptions {
                targets: None,
                jsx: None,
                typescript: None,
                oxc: Some(json!({ "cwd": "/tmp" })),
            };

        let error: napi::Error = to_transform_options(options)
            .expect_err("unsupported oxc field errors");

        assert_eq!(error.status, napi::Status::InvalidArg);
        assert!(error.reason.contains("invalid `oxc options`"));
    }
}
