use napi::{Error, Result, Status};

use telarel_plugin_transform::DefineOptions;

/// Parse the `define` options: a `Record<string, string | number | boolean>`.
///
/// `serde_json::Map` sorts keys without the `preserve_order` feature, so
/// entries arrive in alphabetical key order; duplicate keys cannot reach the
/// binding through the Record shape, so the first-match-wins rule stays
/// unreachable and the ordering is deterministic.
pub fn parse_define(value: serde_json::Value) -> Result<DefineOptions> {
    let map: serde_json::Map<String, serde_json::Value> = match value {
        | serde_json::Value::Object(map) => map,
        | _ => {
            return Err(Error::new(
                Status::InvalidArg,
                String::from("invalid `define options`: expected an object"),
            ));
        },
    };

    let mut entries: Vec<(String, String)> = Vec::with_capacity(map.len());

    for (key, value) in map {
        let replacement: String = match value {
            | serde_json::Value::String(replacement) => replacement,
            | serde_json::Value::Number(number) => number.to_string(),
            | serde_json::Value::Bool(flag) => flag.to_string(),
            | _ => {
                return Err(Error::new(
                    Status::InvalidArg,
                    format!(
                        "invalid `define options`: value for {key:?} must be a string, number or boolean"
                    ),
                ));
            },
        };

        entries.push((key, replacement));
    }

    Ok(DefineOptions { entries })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use telarel_plugin_transform::DefineOptions;

    use super::parse_define;

    #[test]
    fn test_parse_define_scalars_stringified() {
        let parsed: DefineOptions = parse_define(json!({
            "__DEV__": "false",
            "__FLAG__": true,
            "__RATIO__": 1.5,
            "__COUNT__": 42,
        }))
        .expect("define parses");

        assert_eq!(
            parsed.entries,
            vec![
                (String::from("__COUNT__"), String::from("42")),
                (String::from("__DEV__"), String::from("false")),
                (String::from("__FLAG__"), String::from("true")),
                (String::from("__RATIO__"), String::from("1.5")),
            ]
        );
    }

    #[test]
    fn test_parse_define_map_keys_sorted() {
        let parsed: DefineOptions = parse_define(json!({
            "zulu": "1",
            "alpha": "2",
            "mid": "3",
        }))
        .expect("define parses");

        let keys: Vec<&str> = parsed
            .entries
            .iter()
            .map(|(key, _): &(String, String)| key.as_str())
            .collect();

        assert_eq!(keys, vec!["alpha", "mid", "zulu"]);
    }

    #[test]
    fn test_parse_define_non_scalar_value_errors() {
        let error: napi::Error = parse_define(json!({ "__DEV__": ["false"] }))
            .expect_err("non-scalar errors");

        assert_eq!(error.status, napi::Status::InvalidArg);
        assert!(error.reason.contains("invalid `define options`"));
        assert!(error.reason.contains("__DEV__"));
    }

    #[test]
    fn test_parse_define_non_object_errors() {
        let error: napi::Error =
            parse_define(json!(["nope"])).expect_err("non-object errors");

        assert_eq!(error.status, napi::Status::InvalidArg);
        assert!(error.reason.contains("invalid `define options`"));
    }
}
