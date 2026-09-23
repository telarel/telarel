pub mod common;
pub mod transform;

use napi::{Error, Result, Status};

use telarel_plugin::SharedPluginable;

use crate::plugin::builtin::transform::{TRANSFORM_NAME, to_transform_plugin};

/// The napi-visible names of every supported builtin plugin.
#[napi_derive::napi(string_enum)]
pub enum BindingBuiltinPluginName {
    #[napi(value = "builtin:transform")]
    Transform,
}

/// Every builtin name accepted by [`parse_builtin_name`].
const SUPPORTED_BUILTIN_NAMES: [&str; 1] = [TRANSFORM_NAME];

/// Parse a `builtin:<id>` name into its enum variant.
fn parse_builtin_name(name: &str) -> Result<BindingBuiltinPluginName> {
    match name {
        | TRANSFORM_NAME => Ok(BindingBuiltinPluginName::Transform),
        | _ => Err(Error::new(
            Status::InvalidArg,
            format!(
                "unknown builtin plugin {name:?}; supported builtins: {}",
                SUPPORTED_BUILTIN_NAMES.join(", "),
            ),
        )),
    }
}

/// Dispatch a plugin marker by name.
pub fn to_builtin_plugin(
    name: &str,
    options: Option<serde_json::Value>,
) -> Result<SharedPluginable> {
    let builtin: BindingBuiltinPluginName = parse_builtin_name(name)?;

    let plugin: SharedPluginable = match builtin {
        | BindingBuiltinPluginName::Transform => to_transform_plugin(options)?,
    };

    Ok(plugin)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_builtin_plugin_unknown_name_errors() {
        let error: napi::Error = to_builtin_plugin("builtin:nope", None)
            .expect_err("unknown builtin errors");

        assert_eq!(error.status, napi::Status::InvalidArg);
        assert!(error.reason.contains("supported builtins"));
        assert!(error.reason.contains("builtin:transform"));
    }

    #[test]
    fn test_to_builtin_plugin_transform_with_options() {
        let plugin: SharedPluginable = to_builtin_plugin(
            "builtin:transform",
            Some(serde_json::json!({ "targets": ["es2016"] })),
        )
        .expect("builtin:transform dispatches");

        assert_eq!(plugin.call_name(), telarel_plugin_transform::NAME);
    }
}
