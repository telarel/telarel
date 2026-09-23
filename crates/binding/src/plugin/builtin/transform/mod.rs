pub mod options;
pub mod oxc;
pub mod target;

use napi::Result;

use telarel_plugin::SharedPluginable;
use telarel_plugin_transform::TransformOptions;

use crate::plugin::builtin::common::parse_options;
use crate::plugin::builtin::transform::options::{
    BindingTransformPluginOptions, to_transform_options,
};

/// The builtin transform plugin's name.
pub const TRANSFORM_NAME: &str = "builtin:transform";

/// Deserialize the marker's raw `options` JSON and construct the plugin.
pub fn to_transform_plugin(
    options: Option<serde_json::Value>
) -> Result<SharedPluginable> {
    let plugin_options: TransformOptions = match options {
        | Some(value) => {
            let binding_options: BindingTransformPluginOptions =
                parse_options(value, "builtin plugin options")?;

            to_transform_options(binding_options)?
        },
        | None => TransformOptions::default(),
    };

    let plugin: SharedPluginable = telarel_plugin::Plugin::new_shared(
        telarel_plugin_transform::TransformPlugin::with_options(plugin_options),
    );

    Ok(plugin)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use telarel_plugin_transform::NAME;

    use super::{TRANSFORM_NAME, to_transform_plugin};

    #[test]
    fn test_binding_transform_name_matches_crate_constant() {
        assert_eq!(TRANSFORM_NAME, NAME);
    }

    #[test]
    fn test_to_transform_plugin_without_options() {
        let plugin: telarel_plugin::SharedPluginable =
            to_transform_plugin(None).expect("transform dispatches");

        assert_eq!(plugin.call_name(), NAME);
        assert!(
            plugin
                .call_register_hook_usage()
                .contains(telarel_common::HookUsage::Transform)
        );
    }

    #[test]
    fn test_to_transform_plugin_with_options() {
        let plugin: telarel_plugin::SharedPluginable =
            to_transform_plugin(Some(json!({ "targets": ["es2016"] })))
                .expect("transform dispatches");

        assert_eq!(plugin.call_name(), NAME);
    }

    #[test]
    fn test_to_transform_plugin_invalid_options_errors() {
        let error: napi::Error = to_transform_plugin(Some(json!({
            "targets": ["not-a-target"],
        })))
        .expect_err("invalid target errors");

        assert_eq!(error.status, napi::Status::InvalidArg);
    }
}
