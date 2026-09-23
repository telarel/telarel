use telarel_plugin_transform::TransformTarget;

/// Parse a one-key engine object into its target.
fn parse_engine_target(
    key: &str,
    version: &serde_json::Value,
) -> std::result::Result<TransformTarget, String> {
    let major: u64 = version.as_u64().ok_or_else(|| {
        format!(
            "engine `{key}` version must be a non-negative integer, got {version}"
        )
    })?;

    let major: u32 =
        u32::try_from(major).map_err(|error: std::num::TryFromIntError| {
            format!("engine `{key}` version {major} is out of range: {error}")
        })?;

    match key {
        | "chrome" => Ok(TransformTarget::Chrome(major)),
        | "deno" => Ok(TransformTarget::Deno(major)),
        | "edge" => Ok(TransformTarget::Edge(major)),
        | "firefox" => Ok(TransformTarget::Firefox(major)),
        | "ios" => Ok(TransformTarget::Ios(major)),
        | "node" => Ok(TransformTarget::Node(major)),
        | "opera" => Ok(TransformTarget::Opera(major)),
        | "safari" => Ok(TransformTarget::Safari(major)),
        | "samsung" => Ok(TransformTarget::Samsung(major)),
        | "electron" => Ok(TransformTarget::Electron(major)),
        | _ => Err(format!(
            "unknown engine `{key}`, expected one of chrome, deno, edge, firefox, ios, node, opera, safari, samsung, electron"
        )),
    }
}

/// Parse a target string, case-insensitively.
pub fn parse_target_str(
    text: &str
) -> std::result::Result<TransformTarget, String> {
    let lowered: String = text.to_ascii_lowercase();

    match lowered.as_str() {
        | "es2015" => Ok(TransformTarget::Es2015),
        | "es2016" => Ok(TransformTarget::Es2016),
        | "es2017" => Ok(TransformTarget::Es2017),
        | "es2018" => Ok(TransformTarget::Es2018),
        | "es2019" => Ok(TransformTarget::Es2019),
        | "es2020" => Ok(TransformTarget::Es2020),
        | "es2021" => Ok(TransformTarget::Es2021),
        | "es2022" => Ok(TransformTarget::Es2022),
        | "es2023" => Ok(TransformTarget::Es2023),
        | "es2024" => Ok(TransformTarget::Es2024),
        | "es2025" => Ok(TransformTarget::Es2025),
        | "es2026" => Ok(TransformTarget::Es2026),
        | "esnext" => Ok(TransformTarget::EsNext),
        | "hermes" => Ok(TransformTarget::Hermes),
        | "rhino" => Ok(TransformTarget::Rhino),
        | _ => Err(format!(
            "invalid transform target {text:?}: expected \"es2015\"..\"es2026\", \"esnext\", \"hermes\" or \"rhino\""
        )),
    }
}

/// Parse one target from its JSON value.
pub fn parse_target(
    value: &serde_json::Value
) -> std::result::Result<TransformTarget, String> {
    match value {
        | serde_json::Value::String(text) => parse_target_str(text),
        | serde_json::Value::Object(map) if map.len() == 1 => {
            let (key, version): (&String, &serde_json::Value) =
                map.iter().next().expect("one-key object");

            parse_engine_target(key, version).map_err(|message: String| {
                format!("invalid transform target {value}: {message}")
            })
        },
        | _ => Err(format!(
            "invalid transform target {value}: expected a string (`\"es2022\"`, `\"esnext\"`, `\"hermes\"`, `\"rhino\"`) or a one-key engine object like `{{ \"chrome\": 100 }}`"
        )),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use telarel_plugin_transform::TransformTarget;

    use super::{parse_target, parse_target_str};

    #[test]
    fn test_parse_target_str_es_years() {
        let expected: [(u16, TransformTarget); 12] = [
            (2015, TransformTarget::Es2015),
            (2016, TransformTarget::Es2016),
            (2017, TransformTarget::Es2017),
            (2018, TransformTarget::Es2018),
            (2019, TransformTarget::Es2019),
            (2020, TransformTarget::Es2020),
            (2021, TransformTarget::Es2021),
            (2022, TransformTarget::Es2022),
            (2023, TransformTarget::Es2023),
            (2024, TransformTarget::Es2024),
            (2025, TransformTarget::Es2025),
            (2026, TransformTarget::Es2026),
        ];

        for (year, target) in expected {
            let text: String = format!("es{year}");

            assert_eq!(parse_target_str(&text), Ok(target), "es{year}");
        }
    }

    #[test]
    fn test_parse_target_str_esnext() {
        assert_eq!(parse_target_str("esnext"), Ok(TransformTarget::EsNext));
    }

    #[test]
    fn test_parse_target_str_hermes() {
        assert_eq!(parse_target_str("hermes"), Ok(TransformTarget::Hermes));
    }

    #[test]
    fn test_parse_target_str_rhino() {
        assert_eq!(parse_target_str("rhino"), Ok(TransformTarget::Rhino));
    }

    #[test]
    fn test_parse_target_str_mixed_case() {
        assert_eq!(parse_target_str("ES2020"), Ok(TransformTarget::Es2020));
        assert_eq!(parse_target_str("EsNext"), Ok(TransformTarget::EsNext));
        assert_eq!(parse_target_str("HERMES"), Ok(TransformTarget::Hermes));
    }

    #[test]
    fn test_parse_target_str_invalid_string() {
        assert!(parse_target_str("chrome58").is_err());
        assert!(parse_target_str("es2014").is_err());
        assert!(parse_target_str("es2027").is_err());
        assert!(parse_target_str("").is_err());
    }

    #[test]
    fn test_parse_target_engine_objects() {
        let expected: [(&str, u32, TransformTarget, TransformTarget); 10] = [
            (
                "chrome",
                100,
                TransformTarget::Chrome(100),
                TransformTarget::Chrome(0),
            ),
            ("deno", 2, TransformTarget::Deno(2), TransformTarget::Deno(0)),
            ("edge", 91, TransformTarget::Edge(91), TransformTarget::Edge(0)),
            (
                "firefox",
                74,
                TransformTarget::Firefox(74),
                TransformTarget::Firefox(0),
            ),
            ("ios", 13, TransformTarget::Ios(13), TransformTarget::Ios(0)),
            ("node", 22, TransformTarget::Node(22), TransformTarget::Node(0)),
            (
                "opera",
                67,
                TransformTarget::Opera(67),
                TransformTarget::Opera(0),
            ),
            (
                "safari",
                13,
                TransformTarget::Safari(13),
                TransformTarget::Safari(0),
            ),
            (
                "samsung",
                16,
                TransformTarget::Samsung(16),
                TransformTarget::Samsung(0),
            ),
            (
                "electron",
                13,
                TransformTarget::Electron(13),
                TransformTarget::Electron(0),
            ),
        ];

        for (engine, version, target, zero_target) in expected {
            let value: serde_json::Value = json!({ engine: version });

            assert_eq!(parse_target(&value), Ok(target), "{engine}");

            let zero: serde_json::Value = json!({ engine: 0 });

            assert_eq!(parse_target(&zero), Ok(zero_target), "{engine} zero");
        }
    }

    #[test]
    fn test_parse_target_multi_key_object_errors() {
        let multi_key: serde_json::Value = json!({ "chrome": 100, "node": 22 });

        assert!(parse_target(&multi_key).is_err());
    }

    #[test]
    fn test_parse_target_non_number_engine_value_errors() {
        let string_version: serde_json::Value = json!({ "chrome": "100" });

        assert!(parse_target(&string_version).is_err());

        let float_version: serde_json::Value = json!({ "chrome": 100.5 });

        assert!(parse_target(&float_version).is_err());

        let negative_version: serde_json::Value = json!({ "chrome": -1 });

        assert!(parse_target(&negative_version).is_err());
    }

    #[test]
    fn test_parse_target_unknown_engine_errors() {
        let unknown_engine: serde_json::Value = json!({ "browser": 1 });

        assert!(parse_target(&unknown_engine).is_err());
    }

    #[test]
    fn test_parse_target_invalid_types_errors() {
        assert!(parse_target(&serde_json::Value::Null).is_err());
        assert!(parse_target(&json!(123)).is_err());
        assert!(parse_target(&json!([])).is_err());
        assert!(parse_target(&json!({})).is_err());
    }
}
