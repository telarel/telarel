use napi::bindgen_prelude::Object;
use napi::{Error, Result, Status};

use telarel_plugin::{PluginHookMeta, PluginOrder};

/// The order a JS hook requests, parsed from the raw `<hook>Meta.order` string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingPluginOrder {
    /// Runs before the normal bucket.
    Pre,
    /// Runs after the normal bucket.
    Post,
}

/// The raw `<hook>Meta` shape read from a JS plugin object.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BindingPluginHookMeta {
    /// The requested bucket; `None` means the normal bucket.
    pub order: Option<BindingPluginOrder>,
}

/// Parse the raw `order` string. Unknown values are a hard error.
pub fn parse_order(text: &str) -> Result<BindingPluginOrder> {
    match text {
        | "pre" => Ok(BindingPluginOrder::Pre),
        | "post" => Ok(BindingPluginOrder::Post),
        | _ => Err(Error::new(
            Status::InvalidArg,
            format!(
                "unknown plugin order {text:?}: expected \"pre\" or \"post\""
            ),
        )),
    }
}

/// Map the binding meta onto the plugin-layer ordering meta.
/// `None` (field absent) and `order: None` both mean the normal bucket.
pub fn to_plugin_hook_meta(
    meta: Option<BindingPluginHookMeta>
) -> Option<PluginHookMeta> {
    meta.and_then(|meta| meta.order).map(|order| PluginHookMeta {
        order: Some(match order {
            | BindingPluginOrder::Pre => PluginOrder::Pre,
            | BindingPluginOrder::Post => PluginOrder::Post,
        }),
    })
}

/// Read `<hook>Meta` from a raw plugin object and map it to the plugin-layer
/// meta. A present meta object without `order` (or a missing meta) is normal.
pub fn read_hook_meta(
    object: &Object<'static>,
    hook: &str,
) -> Result<Option<PluginHookMeta>> {
    let meta_name: String = format!("{hook}Meta");

    let Some(meta) = object.get::<Object<'_>>(&meta_name)? else {
        return Ok(None);
    };

    let order: Option<String> = meta.get::<String>("order")?;

    let order: Option<BindingPluginOrder> = match order {
        | Some(text) => Some(parse_order(&text)?),
        | None => None,
    };

    Ok(to_plugin_hook_meta(Some(BindingPluginHookMeta { order })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_order_known_values() {
        assert_eq!(parse_order("pre").unwrap(), BindingPluginOrder::Pre);
        assert_eq!(parse_order("post").unwrap(), BindingPluginOrder::Post);
    }

    #[test]
    fn test_parse_order_unknown_values_error() {
        for text in ["", "Pre", "pin-post", "bogus"] {
            let error: napi::Error =
                parse_order(text).expect_err("an unknown order is rejected");

            assert_eq!(error.status, Status::InvalidArg, "{text:?}");
            assert!(
                error.reason.contains("unknown plugin order"),
                "{text:?}: {error}"
            );
        }
    }

    #[test]
    fn test_to_plugin_hook_meta_absent_is_normal() {
        assert_eq!(to_plugin_hook_meta(None), None);
        assert_eq!(
            to_plugin_hook_meta(Some(BindingPluginHookMeta { order: None })),
            None
        );
    }

    #[test]
    fn test_to_plugin_hook_meta_maps_orders() {
        assert_eq!(
            to_plugin_hook_meta(Some(BindingPluginHookMeta {
                order: Some(BindingPluginOrder::Pre),
            })),
            Some(PluginHookMeta { order: Some(PluginOrder::Pre) })
        );
        assert_eq!(
            to_plugin_hook_meta(Some(BindingPluginHookMeta {
                order: Some(BindingPluginOrder::Post),
            })),
            Some(PluginHookMeta { order: Some(PluginOrder::Post) })
        );
    }
}
