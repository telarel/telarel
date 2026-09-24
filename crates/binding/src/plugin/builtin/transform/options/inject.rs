use napi::{Error, Result, Status};

use telarel_plugin_transform::{InjectEntry, InjectOptions, InjectSpecifier};

/// The JS-facing inject specifier mode.
#[derive(Debug, Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
enum BindingInjectMode {
    Named,
    Default,
    Namespace,
}

/// The JS-facing inject object form: `{ source, imported?, local, mode? }`.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct BindingInjectObject {
    source: String,
    imported: Option<String>,
    local: String,
    mode: Option<BindingInjectMode>,
}

/// Parse the inject pair form `[source, imported]` into a named specifier
/// bound to the Record key.
fn parse_inject_pair(
    local: &str,
    pair: Vec<serde_json::Value>,
) -> Result<(String, InjectSpecifier)> {
    let pair: (String, InjectSpecifier) = match pair.as_slice() {
        | [
            serde_json::Value::String(source),
            serde_json::Value::String(imported),
        ] => (
            source.clone(),
            InjectSpecifier::Named {
                imported: Some(imported.clone()),
                local: String::from(local),
            },
        ),
        | _ => {
            return Err(Error::new(
                Status::InvalidArg,
                format!(
                    "invalid `inject options`: value for {local:?} must be a [source, imported] pair of strings"
                ),
            ));
        },
    };

    Ok(pair)
}

/// Parse the inject object form into its source and specifier.
fn parse_inject_object(
    local: &str,
    object: serde_json::Map<String, serde_json::Value>,
) -> Result<(String, InjectSpecifier)> {
    let parsed: BindingInjectObject = serde_json::from_value(
        serde_json::Value::Object(object),
    )
    .map_err(|error: serde_json::Error| {
        Error::new(
            Status::InvalidArg,
            format!("invalid `inject options`: value for {local:?}: {error}"),
        )
    })?;

    let mode: BindingInjectMode =
        parsed.mode.unwrap_or(BindingInjectMode::Named);

    if parsed.imported.is_some() && !matches!(mode, BindingInjectMode::Named) {
        return Err(Error::new(
            Status::InvalidArg,
            format!(
                "invalid `inject options`: value for {local:?} sets `imported`, which only applies to `mode: \"named\"`"
            ),
        ));
    }

    let specifier: InjectSpecifier = match mode {
        | BindingInjectMode::Named => InjectSpecifier::Named {
            imported: parsed.imported,
            local: parsed.local,
        },
        | BindingInjectMode::Default => {
            InjectSpecifier::Default { local: parsed.local }
        },
        | BindingInjectMode::Namespace => {
            InjectSpecifier::Namespace { local: parsed.local }
        },
    };

    Ok((parsed.source, specifier))
}

/// Parse the `inject` options:
/// a `Record<string, string | [string, string] | { source, imported?, local, mode? }>`.
///
/// Entry order carries no semantics (no first-match-wins), so the sorted
/// `serde_json::Map` iteration is fine.
pub fn parse_inject(value: serde_json::Value) -> Result<InjectOptions> {
    let map: serde_json::Map<String, serde_json::Value> = match value {
        | serde_json::Value::Object(map) => map,
        | _ => {
            return Err(Error::new(
                Status::InvalidArg,
                String::from("invalid `inject options`: expected an object"),
            ));
        },
    };

    let mut entries: Vec<InjectEntry> = Vec::with_capacity(map.len());

    for (local, value) in map {
        let entry: InjectEntry = match value {
            | serde_json::Value::String(source) => InjectEntry {
                source,
                specifier: InjectSpecifier::Named {
                    imported: None,
                    local: String::from(&local),
                },
            },
            | serde_json::Value::Array(pair) => {
                let (source, specifier): (String, InjectSpecifier) =
                    parse_inject_pair(&local, pair)?;

                InjectEntry { source, specifier }
            },
            | serde_json::Value::Object(object) => {
                let (source, specifier): (String, InjectSpecifier) =
                    parse_inject_object(&local, object)?;

                InjectEntry { source, specifier }
            },
            | _ => {
                return Err(Error::new(
                    Status::InvalidArg,
                    format!(
                        "invalid `inject options`: value for {local:?} must be a string, [source, imported] pair or object"
                    ),
                ));
            },
        };

        entries.push(entry);
    }

    Ok(InjectOptions { entries })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use telarel_plugin_transform::{InjectOptions, InjectSpecifier};

    use super::parse_inject;

    #[test]
    fn test_parse_inject_string_shape() {
        let parsed: InjectOptions =
            parse_inject(json!({ "$": "jquery" })).expect("inject parses");

        assert_eq!(parsed.entries.len(), 1);
        assert_eq!(parsed.entries[0].source, "jquery");
        assert!(matches!(
            parsed.entries[0].specifier,
            InjectSpecifier::Named { imported: None, .. }
        ));
        assert!(
            matches!(&parsed.entries[0].specifier, InjectSpecifier::Named { local, .. } if local == "$")
        );
    }

    #[test]
    fn test_parse_inject_pair_shape() {
        let parsed: InjectOptions =
            parse_inject(json!({ "$": ["jquery", "default"] }))
                .expect("inject parses");

        assert_eq!(parsed.entries[0].source, "jquery");
        assert!(
            matches!(&parsed.entries[0].specifier, InjectSpecifier::Named { imported: Some(imported), local } if imported == "default" && local == "$")
        );
    }

    #[test]
    fn test_parse_inject_object_shapes() {
        let parsed: InjectOptions = parse_inject(json!({
            "$": {
                "source": "jquery",
                "imported": "default",
                "local": "$",
            },
            "Buffer": { "source": "buffer", "local": "Buffer", "mode": "default" },
            "Process": {
                "source": "process",
                "local": "Process",
                "mode": "namespace",
            },
        }))
        .expect("inject parses");

        let entry: &telarel_plugin_transform::InjectEntry = parsed
            .entries
            .iter()
            .find(|entry: &&telarel_plugin_transform::InjectEntry| {
                entry.source == "jquery"
            })
            .expect("jquery entry");

        assert!(
            matches!(&entry.specifier, InjectSpecifier::Named { imported: Some(imported), local } if imported == "default" && local == "$")
        );

        let entry: &telarel_plugin_transform::InjectEntry = parsed
            .entries
            .iter()
            .find(|entry: &&telarel_plugin_transform::InjectEntry| {
                entry.source == "buffer"
            })
            .expect("buffer entry");

        assert!(matches!(
            &entry.specifier,
            InjectSpecifier::Default { local } if local == "Buffer"
        ));

        let entry: &telarel_plugin_transform::InjectEntry = parsed
            .entries
            .iter()
            .find(|entry: &&telarel_plugin_transform::InjectEntry| {
                entry.source == "process"
            })
            .expect("process entry");

        assert!(matches!(
            &entry.specifier,
            InjectSpecifier::Namespace { local } if local == "Process"
        ));
    }

    #[test]
    fn test_parse_inject_object_unknown_field_errors() {
        let error: napi::Error = parse_inject(json!({
            "$": { "source": "jquery", "nope": 1 }
        }))
        .expect_err("unknown field errors");

        assert_eq!(error.status, napi::Status::InvalidArg);
        assert!(error.reason.contains("invalid `inject options`"));
        assert!(error.reason.contains("$"), "{}", error.reason);
    }

    #[test]
    fn test_parse_inject_object_unknown_field_names_bad_key() {
        let error: napi::Error = parse_inject(json!({
            "good": { "source": "react", "local": "React" },
            "$": { "nope": 1 },
        }))
        .expect_err("unknown field errors");

        assert_eq!(error.status, napi::Status::InvalidArg);
        assert!(error.reason.contains("invalid `inject options`"));
        assert!(error.reason.contains("$"), "{}", error.reason);
        assert!(error.reason.contains("nope"), "{}", error.reason);
    }

    #[test]
    fn test_parse_inject_object_imported_named() {
        let parsed: InjectOptions = parse_inject(json!({
            "style": {
                "source": "styles",
                "imported": "css",
                "local": "style",
                "mode": "named",
            },
        }))
        .expect("inject parses");

        assert!(matches!(
            &parsed.entries[0].specifier,
            InjectSpecifier::Named { imported: Some(imported), local } if imported == "css" && local == "style"
        ));
    }

    #[test]
    fn test_parse_inject_object_imported_with_default_mode_errors() {
        let error: napi::Error = parse_inject(json!({
            "Buffer": {
                "source": "buffer",
                "imported": "X",
                "local": "Buffer",
                "mode": "default",
            },
        }))
        .expect_err("imported conflict errors");

        assert_eq!(error.status, napi::Status::InvalidArg);
        assert!(error.reason.contains("invalid `inject options`"));
        assert!(error.reason.contains("imported"), "{}", error.reason);
        assert!(error.reason.contains("Buffer"), "{}", error.reason);
    }

    #[test]
    fn test_parse_inject_object_missing_local_errors() {
        let error: napi::Error =
            parse_inject(json!({ "$": { "source": "jquery" } }))
                .expect_err("missing local errors");

        assert_eq!(error.status, napi::Status::InvalidArg);
        assert!(error.reason.contains("invalid `inject options`"));
    }

    #[test]
    fn test_parse_inject_invalid_mode_errors() {
        let error: napi::Error = parse_inject(json!({
            "$": { "source": "jquery", "local": "$", "mode": "star" },
        }))
        .expect_err("invalid mode errors");

        assert_eq!(error.status, napi::Status::InvalidArg);
        assert!(error.reason.contains("invalid `inject options`"));
    }

    #[test]
    fn test_parse_inject_invalid_shape_errors() {
        let error: napi::Error =
            parse_inject(json!({ "$": 42 })).expect_err("invalid shape errors");

        assert_eq!(error.status, napi::Status::InvalidArg);
        assert!(error.reason.contains("invalid `inject options`"));
        assert!(error.reason.contains("$"));
    }

    #[test]
    fn test_parse_inject_non_object_errors() {
        let error: napi::Error =
            parse_inject(json!("nope")).expect_err("non-object errors");

        assert_eq!(error.status, napi::Status::InvalidArg);
        assert!(error.reason.contains("invalid `inject options`"));
    }
}
