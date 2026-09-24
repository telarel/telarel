pub mod define;
pub mod inject;
pub mod oxc;
pub mod target;

use napi::{Error, Result, Status};

use telarel_plugin_transform::{
    DefineOptions, InjectOptions, JsxOptions as TelarelJsxOptions,
    TransformOptions, TransformTarget,
    TypeScriptOptions as TelarelTypeScriptOptions,
};

use crate::plugin::builtin::common::parse_optional_options;
use crate::plugin::builtin::transform::options::define::parse_define;
use crate::plugin::builtin::transform::options::inject::parse_inject;
use crate::plugin::builtin::transform::options::oxc::OxcPassthrough;
use crate::plugin::builtin::transform::options::target::parse_target;

/// The JS-facing options bag for the builtin transform plugin.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BindingTransformPluginOptions {
    pub targets: Option<Vec<serde_json::Value>>,
    pub typescript: Option<serde_json::Value>,
    pub jsx: Option<serde_json::Value>,
    pub inject: Option<serde_json::Value>,
    pub define: Option<serde_json::Value>,
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

    let typescript: Option<TelarelTypeScriptOptions> =
        parse_optional_options(options.typescript, "typescript options")?;

    let jsx: Option<TelarelJsxOptions> =
        parse_optional_options(options.jsx, "jsx options")?;

    let inject: Option<InjectOptions> =
        options.inject.map(parse_inject).transpose()?;

    let define: Option<DefineOptions> =
        options.define.map(parse_define).transpose()?;

    let oxc: Option<OxcPassthrough> =
        parse_optional_options(options.oxc, "oxc options")?;

    Ok(TransformOptions {
        targets: parsed,
        typescript,
        jsx,
        inject,
        define,
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

    fn fixtures(
        targets: Option<Vec<serde_json::Value>>,
        typescript: Option<serde_json::Value>,
        jsx: Option<serde_json::Value>,
        inject: Option<serde_json::Value>,
        define: Option<serde_json::Value>,
        oxc: Option<serde_json::Value>,
    ) -> BindingTransformPluginOptions {
        BindingTransformPluginOptions {
            targets,
            typescript,
            jsx,
            inject,
            define,
            oxc,
        }
    }

    #[test]
    fn test_to_transform_options_defaults() {
        let options: BindingTransformPluginOptions =
            fixtures(None, None, None, None, None, None);

        let parsed: TelarelTransformOptions =
            to_transform_options(options).expect("defaults parse");

        assert!(parsed.targets.is_empty());
        assert!(parsed.jsx.is_none());
        assert!(parsed.typescript.is_none());
        assert!(parsed.define.is_none());
        assert!(parsed.inject.is_none());
        assert!(parsed.oxc.is_none());
    }

    #[test]
    fn test_to_transform_options_targets() {
        let options: BindingTransformPluginOptions = fixtures(
            Some(vec![
                json!("es2022"),
                json!({ "chrome": 100 }),
                json!("ESNEXT"),
            ]),
            None,
            None,
            None,
            None,
            None,
        );

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
        let options: BindingTransformPluginOptions = fixtures(
            Some(vec![json!("chrome58")]),
            None,
            None,
            None,
            None,
            None,
        );

        let error: napi::Error =
            to_transform_options(options).expect_err("invalid target errors");

        assert_eq!(error.status, napi::Status::InvalidArg);
        assert!(error.reason.contains("invalid transform target"));
    }

    #[test]
    fn test_to_transform_options_typescript_and_jsx() {
        let options: BindingTransformPluginOptions = fixtures(
            None,
            Some(json!({
                "onlyRemoveTypeImports": true,
                "jsxPragma": "h",
            })),
            Some(json!({
                "runtime": "classic",
                "importSource": "preact",
            })),
            None,
            None,
            None,
        );

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
        let options: BindingTransformPluginOptions = fixtures(
            None,
            None,
            Some(json!({ "unknownField": true })),
            None,
            None,
            None,
        );

        let error: napi::Error =
            to_transform_options(options).expect_err("unknown field errors");

        assert_eq!(error.status, napi::Status::InvalidArg);
        assert!(error.reason.contains("invalid `jsx options`"));
    }

    #[test]
    fn test_to_transform_options_oxc_passthrough() {
        let options: BindingTransformPluginOptions = fixtures(
            None,
            None,
            None,
            None,
            None,
            Some(json!({
                "jsx": { "pragma": "h" },
                "env": { "targets": { "chrome": "80" } },
            })),
        );

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
        let options: BindingTransformPluginOptions = fixtures(
            None,
            None,
            None,
            None,
            None,
            Some(json!({ "cwd": "/tmp" })),
        );

        let error: napi::Error = to_transform_options(options)
            .expect_err("unsupported oxc field errors");

        assert_eq!(error.status, napi::Status::InvalidArg);
        assert!(error.reason.contains("invalid `oxc options`"));
    }

    #[test]
    fn test_to_transform_options_define_and_inject_combined() {
        let options: BindingTransformPluginOptions = fixtures(
            None,
            None,
            None,
            Some(json!({ "$": "jquery" })),
            Some(json!({ "__DEV__": "false" })),
            None,
        );

        let parsed: TelarelTransformOptions =
            to_transform_options(options).expect("define and inject parse");

        assert!(parsed.define.is_some());
        assert!(parsed.inject.is_some());
    }
}
