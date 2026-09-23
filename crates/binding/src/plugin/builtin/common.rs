use napi::{Error, Result, Status};

/// Build a napi `InvalidArg` error for a JSON deserialization failure.
fn invalid_arg(
    context: &str,
    error: serde_json::Error,
) -> Error {
    Error::new(Status::InvalidArg, format!("invalid {context}: {error}"))
}

/// Parse a plugin-options JSON value into `T`,
/// mapping errors to napi `InvalidArg`.
pub fn parse_options<T: serde::de::DeserializeOwned>(
    value: serde_json::Value,
    context: &str,
) -> Result<T> {
    serde_json::from_value::<T>(value).map_err(|error: serde_json::Error| {
        invalid_arg(&format!("`{context}`"), error)
    })
}

/// Parse an optional plugin-options JSON layer into `T`.
pub fn parse_optional_options<T: serde::de::DeserializeOwned>(
    value: Option<serde_json::Value>,
    context: &str,
) -> Result<Option<T>> {
    let Some(value) = value else {
        return Ok(None);
    };

    parse_options::<T>(value, context).map(Some)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{parse_optional_options, parse_options};

    #[test]
    fn test_parse_options_parses_value() {
        let parsed: u32 =
            parse_options(json!(42), "thing").expect("value parses");

        assert_eq!(parsed, 42);
    }

    #[test]
    fn test_parse_options_errors_with_context() {
        let error: napi::Error = parse_options::<u32>(json!("nope"), "thing")
            .expect_err("invalid value errors");

        assert_eq!(error.status, napi::Status::InvalidArg);
        assert!(error.reason.contains("invalid `thing`"));
    }

    #[test]
    fn test_parse_optional_options_none_is_none() {
        let parsed: Option<u32> =
            parse_optional_options(None, "layer options").expect("none passes");

        assert!(parsed.is_none());
    }

    #[test]
    fn test_parse_optional_options_parses_some() {
        let parsed: Option<u32> =
            parse_optional_options(Some(json!(7)), "layer options")
                .expect("value parses");

        assert_eq!(parsed, Some(7));
    }

    #[test]
    fn test_parse_optional_options_errors_with_context() {
        let error: napi::Error =
            parse_optional_options::<u32>(Some(json!("nope")), "layer options")
                .expect_err("invalid layer errors");

        assert_eq!(error.status, napi::Status::InvalidArg);
        assert!(error.reason.contains("invalid `layer options`"));
    }
}
