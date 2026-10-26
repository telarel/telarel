use napi::bindgen_prelude::{Object, ToNapiValue, Unknown};
use napi::{Env, Error, Result, Status};
use serde::{Deserialize, Serialize};
use telarel_common::CompileError;

use crate::errors::{ErrorSourcePayload, ErrorSpanPayload};
use crate::plugin::spans::SpanMap;

/// The private marker that prefixes encoded compile errors.
const COMPILE_ERROR_MARKER: &str = "__TELAREL_COMPILE_ERROR__:";

/// The error payload that [`COMPILE_ERROR_MARKER`] carries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompileErrorPayload {
    /// The JS error class name.
    pub name: String,
    /// The bare error message.
    pub message: String,
    /// The source location, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<ErrorSourcePayload>,
}

/// Encode a compile error into its marker payload.
///
/// The function returns `None` when serialization fails. The caller then
/// falls back to the plain message.
pub fn encode_compile_error(error: &CompileError) -> Option<String> {
    let source: Option<ErrorSourcePayload> = error.source().map(|source| {
        let map: SpanMap = SpanMap::new(source.code());

        ErrorSourcePayload {
            file: source.file().to_owned(),
            code: source.code().to_owned(),
            span: ErrorSpanPayload {
                start: u32::try_from(
                    map.byte_to_utf16(source.span().start as usize),
                )
                .unwrap_or(u32::MAX),
                end: u32::try_from(
                    map.byte_to_utf16(source.span().end as usize),
                )
                .unwrap_or(u32::MAX),
            },
        }
    });

    let payload: CompileErrorPayload = CompileErrorPayload {
        name: "CompileError".to_owned(),
        message: error.message().to_owned(),
        source,
    };

    serde_json::to_string(&payload)
        .ok()
        .map(|json: String| format!("{COMPILE_ERROR_MARKER}{json}"))
}

/// Decode a compile error payload from an error reason.
///
/// The function returns `None` when the marker is absent or the payload is
/// malformed, so callers can rethrow the original error unchanged.
pub fn decode_compile_error(reason: &str) -> Option<CompileErrorPayload> {
    let reason: &str = reason.strip_prefix("Error: ").unwrap_or(reason);

    let payload: &str = reason.strip_prefix(COMPILE_ERROR_MARKER)?;

    let payload: CompileErrorPayload = serde_json::from_str(payload).ok()?;

    if payload.name != "CompileError" {
        return None;
    }

    Some(payload)
}

/// Rebuild the encoded compile error as a JS `Error` on the JS thread.
pub fn reject_compile_error(
    env: &Env,
    payload: CompileErrorPayload,
) -> Result<Error> {
    let mut error: Object<'_> = env.create_error(Error::new(
        Status::GenericFailure,
        payload.message.clone(),
    ))?;

    error.set("name", "CompileError")?;

    error.set("message", payload.message)?;

    if let Some(source) = payload.source {
        let mut js_source: Object<'_> = Object::new(env)?;

        js_source.set("file", source.file)?;

        js_source.set("code", source.code)?;

        let mut js_span: Object<'_> = Object::new(env)?;

        js_span.set("start", source.span.start)?;

        js_span.set("end", source.span.end)?;

        js_source.set("span", js_span)?;

        error.set("source", js_source)?;
    }

    let unknown: Unknown = error.into_unknown(env)?;

    Ok(Error::from_unknown_without_coercion(unknown))
}

#[cfg(test)]
mod tests {
    use oxc::span::Span;

    use super::*;

    #[test]
    fn test_encode_source_less_error() {
        let error: CompileError = CompileError::new("boom");
        let encoded: String =
            encode_compile_error(&error).expect("encoding succeeds");

        assert!(encoded.starts_with(COMPILE_ERROR_MARKER));

        let payload: CompileErrorPayload =
            decode_compile_error(&encoded).expect("payload decodes");

        assert_eq!(payload.name, "CompileError");
        assert_eq!(payload.message, "boom");
        assert!(payload.source.is_none());
    }

    #[test]
    fn test_encode_source_bound_error_uses_utf16_offsets() {
        let error: CompileError = CompileError::new("boom").with_source_code(
            "input.ts",
            "😀日",
            Span::new(0, 7),
        );
        let encoded: String =
            encode_compile_error(&error).expect("encoding succeeds");
        let payload: CompileErrorPayload =
            decode_compile_error(&encoded).expect("payload decodes");

        assert_eq!(payload.message, "boom");

        let source: ErrorSourcePayload =
            payload.source.expect("a source is encoded");

        assert_eq!(source.file, "input.ts");
        assert_eq!(source.code, "😀日");

        // The source is 7 bytes but 3 UTF-16 code units.
        assert_eq!((source.span.start, source.span.end), (0, 3));
    }

    #[test]
    fn test_encode_utf16_offsets_for_cjk_and_emoji() {
        let code: &str = "😀日x";
        let error: CompileError = CompileError::new("boom").with_source_code(
            "input.ts",
            code,
            Span::new(0, 4),
        );
        let encoded: String =
            encode_compile_error(&error).expect("encoding succeeds");
        let payload: CompileErrorPayload =
            decode_compile_error(&encoded).expect("payload decodes");
        let source: ErrorSourcePayload =
            payload.source.expect("a source is encoded");

        assert_eq!((source.span.start, source.span.end), (0, 2));

        let error: CompileError = CompileError::new("boom").with_source_code(
            "input.ts",
            code,
            Span::new(4, 7),
        );
        let encoded: String =
            encode_compile_error(&error).expect("encoding succeeds");
        let payload: CompileErrorPayload =
            decode_compile_error(&encoded).expect("payload decodes");
        let source: ErrorSourcePayload =
            payload.source.expect("a source is encoded");

        assert_eq!((source.span.start, source.span.end), (2, 3));
    }

    #[test]
    fn test_malformed_payload_is_ignored() {
        let reason: String = format!("{COMPILE_ERROR_MARKER}{{\"message\":");

        assert!(decode_compile_error(&reason).is_none());
    }

    #[test]
    fn test_unrelated_marker_is_ignored() {
        let reason: String =
            "__TELAREL_UNRELATED__:{\"name\":\"CompileError\",\"message\":\"boom\"}"
                .to_owned();

        assert!(decode_compile_error(&reason).is_none());
    }

    #[test]
    fn test_absent_marker_is_ignored() {
        assert!(decode_compile_error("boom").is_none());
    }

    #[test]
    fn test_tolerates_a_leading_error_prefix() {
        let error: CompileError = CompileError::new("boom");
        let encoded: String =
            encode_compile_error(&error).expect("encoding succeeds");
        let payload: CompileErrorPayload =
            decode_compile_error(&format!("Error: {encoded}"))
                .expect("payload decodes");

        assert_eq!(payload.message, "boom");
    }
}
