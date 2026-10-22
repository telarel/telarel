use base64::Engine;
use base64::engine::general_purpose::STANDARD;

use crate::errors::compile::CompileError;
use crate::sourcemap::SourceMap;

const SOURCE_MAPPING_URL: &str = "sourceMappingURL=";

const DATA_URI_PREFIX: &str = "data:";

const SOURCE_MAPPING_COMMENT_PREFIX: &str = "//# sourceMappingURL=";

/// The last line with non-whitespace content, if any.
fn last_non_empty_line(code: &str) -> Option<&str> {
    code.lines().rev().find(|line: &&str| !line.trim().is_empty())
}

/// The URL following `sourceMappingURL=` on `line`, if present.
fn source_mapping_url(line: &str) -> Option<&str> {
    let marker: usize = line.find(SOURCE_MAPPING_URL)?;
    let rest: &str = &line[marker + SOURCE_MAPPING_URL.len()..];
    let end: usize = rest.find("*/").unwrap_or(rest.len());
    let url: &str = rest[..end].trim();

    (!url.is_empty()).then_some(url)
}

/// The base64 payload of a `data:...;base64,<payload>` URI, if it is one.
fn decode_data_uri_payload(url: &str) -> Option<&str> {
    let rest: &str = url.strip_prefix(DATA_URI_PREFIX)?;
    let comma: usize = rest.find(',')?;
    let (metadata, payload): (&str, &str) = rest.split_at(comma);
    let payload: &str = &payload[1..];
    let is_base64: bool = metadata
        .split(';')
        .any(|part: &str| part.eq_ignore_ascii_case("base64"));

    is_base64.then_some(payload)
}

/// Decode a base64 data-URI payload into an owned source map.
fn decode_payload(payload: &str) -> Result<SourceMap, CompileError> {
    let bytes: Vec<u8> = STANDARD.decode(payload.trim()).map_err(
        |error: base64::DecodeError| {
            CompileError::new(format!(
                "invalid inline source map: invalid base64 payload: {error}"
            ))
        },
    )?;

    let json: &str =
        std::str::from_utf8(&bytes).map_err(|error: std::str::Utf8Error| {
            CompileError::new(format!(
                "invalid inline source map: payload is not UTF-8: {error}"
            ))
        })?;

    let map: oxc_sourcemap::SourceMap<'_> =
        oxc_sourcemap::SourceMap::from_json_string(json).map_err(
            |error: oxc_sourcemap::Error| {
                CompileError::new(format!(
                    "invalid inline source map: invalid map JSON: {error}"
                ))
            },
        )?;

    Ok(map.into_owned())
}

/// Append a `//# sourceMappingURL=<url>` comment to `code`.
///
/// Ensures `code` ends with a newline before appending the comment line.
pub fn append_source_mapping_url(
    mut code: String,
    url: &str,
) -> String {
    if !code.ends_with('\n') {
        code.push('\n');
    }

    code.push_str(SOURCE_MAPPING_COMMENT_PREFIX);
    code.push_str(url);
    code.push('\n');

    code
}

/// Append an inline `//# sourceMappingURL=data:...` comment carrying `map`.
///
/// Ensures `code` ends with a newline before appending the comment line.
pub fn append_inline_source_map(
    code: String,
    map: &SourceMap,
) -> String {
    let payload: String = STANDARD.encode(map.to_json_string());
    let url: String = format!("data:application/json;base64,{payload}");

    append_source_mapping_url(code, &url)
}

/// Extract an inline `sourceMappingURL` data-URI source map from `code`.
///
/// Scans the last non-empty line for a `sourceMappingURL` comment, tolerating
/// the `//#` and `//@` line spellings and the `/*# ... */` block form.
///
/// - `None` when no inline map comment is present.
/// - `Some(Ok(map))` when the data URI decodes and parses.
/// - `Some(Err(error))` when the comment is present but corrupt.
pub fn extract_inline_source_map(
    code: &str
) -> Option<Result<SourceMap, CompileError>> {
    let line: &str = last_non_empty_line(code)?;
    let url: &str = source_mapping_url(line)?;
    let payload: &str = decode_data_uri_payload(url)?;

    Some(decode_payload(payload))
}

#[cfg(test)]
mod tests {
    use oxc_sourcemap::SourceMapBuilder;

    use super::*;

    fn sample_map() -> SourceMap {
        let mut builder: SourceMapBuilder<'_> = SourceMapBuilder::default();

        builder.set_file("original.ts");

        let source_id: u32 =
            builder.add_source_and_content("original.ts", "const a = 1;");

        builder.add_token(0, 0, 3, 2, Some(source_id), None);

        builder.into_owned_sourcemap().into_inner()
    }

    fn data_url(map: &SourceMap) -> String {
        format!(
            "data:application/json;base64,{}",
            STANDARD.encode(map.to_json_string()),
        )
    }

    #[test]
    fn test_extract_absent_returns_none() {
        assert!(extract_inline_source_map("const a = 1;\n").is_none());
        assert!(extract_inline_source_map("").is_none());
    }

    #[test]
    fn test_extract_valid_round_trip() {
        let map: SourceMap = sample_map();

        let code: String =
            append_inline_source_map(String::from("const a = 1;"), &map);

        let extracted: SourceMap = extract_inline_source_map(&code)
            .expect("comment present")
            .expect("valid map");

        assert_eq!(extracted.to_json_string(), map.to_json_string());
    }

    #[test]
    fn test_extract_corrupt_base64_returns_err() {
        let code: &str = "const a = 1;\n//# sourceMappingURL=data:application/json;base64,!!!not-base64!!!\n";

        let error: CompileError = extract_inline_source_map(code)
            .expect("comment present")
            .expect_err("corrupt base64 rejected");

        assert!(
            error.to_string().contains("invalid inline source map"),
            "message names the inline cause: {error}",
        );
    }

    #[test]
    fn test_extract_corrupt_json_returns_err() {
        let payload: String = STANDARD.encode("not valid json");
        let code: String = format!(
            "const a = 1;\n//# sourceMappingURL=data:application/json;base64,{payload}\n",
        );

        let error: CompileError = extract_inline_source_map(&code)
            .expect("comment present")
            .expect_err("corrupt JSON rejected");

        assert!(
            error.to_string().contains("invalid inline source map"),
            "message names the inline cause: {error}",
        );
    }

    #[test]
    fn test_extract_slash_at_spelling() {
        let map: SourceMap = sample_map();
        let code: String =
            format!("const a = 1;\n//@ sourceMappingURL={}\n", data_url(&map));

        let extracted: SourceMap = extract_inline_source_map(&code)
            .expect("comment present")
            .expect("valid map");

        assert_eq!(extracted.to_json_string(), map.to_json_string());
    }

    #[test]
    fn test_extract_block_comment_form() {
        let map: SourceMap = sample_map();
        let code: String = format!(
            "const a = 1;\n/*# sourceMappingURL={} */\n",
            data_url(&map)
        );

        let extracted: SourceMap = extract_inline_source_map(&code)
            .expect("comment present")
            .expect("valid map");

        assert_eq!(extracted.to_json_string(), map.to_json_string());
    }

    #[test]
    fn test_append_source_mapping_url_appends_external_comment() {
        let code: String = append_source_mapping_url(
            String::from("const a = 1;"),
            "dist/app.js.map",
        );

        assert_eq!(
            code,
            "const a = 1;\n//# sourceMappingURL=dist/app.js.map\n",
        );
    }

    #[test]
    fn test_append_without_trailing_newline_adds_one() {
        let map: SourceMap = sample_map();

        let code: String =
            append_inline_source_map(String::from("const a = 1;"), &map);

        assert!(code.starts_with("const a = 1;\n"));
        assert!(code.ends_with('\n'));
    }

    #[test]
    fn test_append_with_trailing_newline_keeps_single_separator() {
        let map: SourceMap = sample_map();

        let code: String =
            append_inline_source_map(String::from("const a = 1;\n"), &map);

        assert!(code.starts_with("const a = 1;\n//# sourceMappingURL="));
    }

    #[test]
    fn test_append_then_extract_matches_map() {
        let map: SourceMap = sample_map();

        for input in ["const a = 1;", "const a = 1;\n"] {
            let code: String =
                append_inline_source_map(String::from(input), &map);

            let extracted: SourceMap = extract_inline_source_map(&code)
                .expect("comment present")
                .expect("valid map");

            assert_eq!(extracted.to_json_string(), map.to_json_string());
        }
    }
}
