use oxc::span::Span;
use serde::Deserialize;
use telarel_common::PluginError;

use crate::errors::ErrorSourcePayload;
use crate::plugin::spans::SpanMap;

/// The private marker a JS `PluginError` encodes into its message.
const PLUGIN_ERROR_MARKER: &str = "__TELAREL_PLUGIN_ERROR__:";

/// The payload carried by [`PLUGIN_ERROR_MARKER`].
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PluginErrorPayload {
    /// The bare error message.
    pub message: String,
    /// The source location, if any.
    #[serde(default)]
    pub source: Option<ErrorSourcePayload>,
}

/// Convert a JS UTF-16 code-unit offset to a clamped UTF-8 byte offset.
fn utf16_to_byte(
    map: &SpanMap,
    offset: u64,
) -> u32 {
    let offset: usize = usize::try_from(offset).unwrap_or(usize::MAX);

    u32::try_from(map.utf16_to_byte(offset)).unwrap_or(u32::MAX)
}

/// Decode a JS `PluginError` envelope carried by an error message.
///
/// Returns `None` when the marker is absent or the payload is malformed, so
/// callers can fall back to the plain message. The payload's UTF-16 span is
/// converted to byte offsets against its own `code` before constructing the
/// [`PluginError`].
pub fn decode_plugin_error(reason: &str) -> Option<PluginError> {
    let reason: &str = reason.strip_prefix("Error: ").unwrap_or(reason);

    let payload: &str = reason.strip_prefix(PLUGIN_ERROR_MARKER)?;

    let payload: PluginErrorPayload = serde_json::from_str(payload).ok()?;

    let error: PluginError = PluginError::new(payload.message);

    let Some(source) = payload.source else {
        return Some(error);
    };

    let map: SpanMap = SpanMap::new(&source.code);

    let start: u32 = utf16_to_byte(&map, u64::from(source.span.start));

    let end: u32 = utf16_to_byte(&map, u64::from(source.span.end));

    let (start, end): (u32, u32) =
        if start <= end { (start, end) } else { (end, start) };

    Some(error.with_source_code(
        source.file,
        source.code,
        Span::new(start, end),
    ))
}

#[cfg(test)]
mod tests {
    use oxc::span::Span;
    use telarel_common::ErrorSource;

    use super::*;

    fn envelope(
        message: &str,
        source: Option<&str>,
    ) -> String {
        match source {
            | Some(source) => format!(
                "{PLUGIN_ERROR_MARKER}{{\"message\":{message:?},\"source\":{source}}}"
            ),
            | None => {
                format!("{PLUGIN_ERROR_MARKER}{{\"message\":{message:?}}}")
            },
        }
    }

    #[test]
    fn test_decodes_source_less_payload() {
        let reason: String = envelope("boom", None);

        let error: PluginError =
            decode_plugin_error(&reason).expect("the envelope decodes");

        assert_eq!(error.message(), "boom");
        assert!(error.source().is_none());
    }

    #[test]
    fn test_decodes_source_payload() {
        let reason: String = envelope(
            "boom",
            Some(
                r#"{"file":"input.ts","code":"const x = 1;","span":{"start":6,"end":7}}"#,
            ),
        );

        let error: PluginError =
            decode_plugin_error(&reason).expect("the envelope decodes");

        assert_eq!(error.message(), "boom");

        let source: &ErrorSource =
            error.source().expect("the source is decoded");

        assert_eq!(source.file(), "input.ts");
        assert_eq!(source.code(), "const x = 1;");
        assert_eq!(source.span(), Span::new(6, 7));
    }

    #[test]
    fn test_decodes_utf16_offsets_for_cjk_and_emoji() {
        let reason: String = envelope(
            "boom",
            Some(
                r#"{"file":"input.ts","code":"😀日","span":{"start":2,"end":3}}"#,
            ),
        );

        let error: PluginError =
            decode_plugin_error(&reason).expect("the envelope decodes");

        let source: &ErrorSource =
            error.source().expect("the source is decoded");

        assert_eq!(source.span(), Span::new(4, 7));

        let reason: String = envelope(
            "boom",
            Some(
                r#"{"file":"input.ts","code":"😀日","span":{"start":0,"end":2}}"#,
            ),
        );

        let error: PluginError =
            decode_plugin_error(&reason).expect("the envelope decodes");

        let source: &ErrorSource =
            error.source().expect("the source is decoded");

        assert_eq!(source.span(), Span::new(0, 4));
    }

    #[test]
    fn test_malformed_json_is_ignored() {
        let reason: String = format!("{PLUGIN_ERROR_MARKER}{{\"message\":");

        assert!(decode_plugin_error(&reason).is_none());
    }

    #[test]
    fn test_missing_message_is_ignored() {
        let reason: String =
            format!("{PLUGIN_ERROR_MARKER}{{\"source\":null}}");

        assert!(decode_plugin_error(&reason).is_none());
    }

    #[test]
    fn test_unrelated_marker_is_ignored() {
        let reason: String =
            "__TELAREL_UNRELATED__:{\"message\":\"boom\"}".to_owned();

        assert!(decode_plugin_error(&reason).is_none());
    }

    #[test]
    fn test_absent_marker_is_ignored() {
        assert!(decode_plugin_error("boom").is_none());
    }

    #[test]
    fn test_tolerates_a_leading_error_prefix() {
        let reason: String = format!("Error: {}", envelope("boom", None));

        let error: PluginError =
            decode_plugin_error(&reason).expect("the envelope decodes");

        assert_eq!(error.message(), "boom");
    }
}
