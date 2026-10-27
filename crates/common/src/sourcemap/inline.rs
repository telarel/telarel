use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use oxc::allocator::Allocator;
use oxc::ast::ast::{Comment, Program};

use crate::_types::options::sourcemap::input::{
    InputSourceMap, InputSourceMapMode,
};
use crate::ast::ast::Ast;
use crate::errors::compile::CompileError;
use crate::sourcemap::SourceMap;

const SOURCE_MAPPING_URL: &str = "sourceMappingURL=";

const DATA_URI_PREFIX: &str = "data:";

const SOURCE_MAPPING_COMMENT_PREFIX: &str = "//# sourceMappingURL=";

fn last_non_empty_line(code: &str) -> Option<(&str, usize)> {
    let trimmed: &str = code.trim_end();

    let line_start: usize =
        trimmed.rfind('\n').map_or(0, |index: usize| index + 1);

    let line: &str = &trimmed[line_start..];

    let line: &str = line.strip_suffix('\r').unwrap_or(line);

    if line.trim().is_empty() {
        return None;
    }

    Some((line, line_start))
}

/// An inline map URL and the byte offset of its `sourceMappingURL=` marker.
struct InlineSourceMapComment<'a> {
    marker: usize,
    url: &'a str,
}

fn source_mapping_url(
    line: &str,
    line_start: usize,
) -> Option<InlineSourceMapComment<'_>> {
    let marker: usize = line.find(SOURCE_MAPPING_URL)?;

    let rest: &str = &line[marker + SOURCE_MAPPING_URL.len()..];

    let end: usize = rest.find("*/").unwrap_or(rest.len());

    let url: &str = rest[..end].trim();

    (!url.is_empty())
        .then_some(InlineSourceMapComment { marker: line_start + marker, url })
}

fn find_inline_source_map_comment(
    code: &str
) -> Option<InlineSourceMapComment<'_>> {
    let (line, line_start): (&str, usize) = last_non_empty_line(code)?;

    source_mapping_url(line, line_start)
}

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
    let comment: InlineSourceMapComment<'_> =
        find_inline_source_map_comment(code)?;

    let payload: &str = decode_data_uri_payload(comment.url)?;

    Some(decode_payload(payload))
}

/// Remove the inline source-map comment that supplied `input`.
///
/// This function considers only `Inline` and `Auto` requests. It removes a
/// comment only when the last non-empty line carries a base64 data-URI
/// `sourceMappingURL` comment. It removes the comment from `Program.comments`
/// and leaves `Ast::source` unchanged. Returns whether it removed a comment.
pub fn strip_consumed_inline_source_map_comment(
    ast: &mut Ast,
    input: &InputSourceMap,
) -> bool {
    let mode: &InputSourceMapMode = input.mode();

    if !matches!(mode, InputSourceMapMode::Inline | InputSourceMapMode::Auto) {
        return false;
    }

    let Some(comment): Option<InlineSourceMapComment<'_>> =
        find_inline_source_map_comment(ast.source().as_ref())
    else {
        return false;
    };

    if decode_data_uri_payload(comment.url).is_none() {
        return false;
    }

    let marker: usize = comment.marker;

    ast.with_mut(|_allocator: &Allocator, program: &mut Program<'_>| -> bool {
        let before: usize = program.comments.len();

        program.comments.retain(|comment: &Comment| {
            !(comment.span.start as usize <= marker
                && marker < comment.span.end as usize)
        });

        program.comments.len() != before
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use oxc_sourcemap::SourceMapBuilder;

    use crate::_types::options::sourcemap::input::InputSourceMapOptions;
    use crate::ast::parse::{ParseAstOptions, parse_ast};
    use crate::helpers::test_util::comment_text;

    use super::*;

    fn sample_map_for(file: &str) -> SourceMap {
        let mut builder: SourceMapBuilder<'_> = SourceMapBuilder::default();

        builder.set_file(file);

        let source_id: u32 =
            builder.add_source_and_content(file, "const a = 1;");

        builder.add_token(0, 0, 3, 2, Some(source_id), None);

        builder.into_owned_sourcemap().into_inner()
    }

    fn sample_map() -> SourceMap {
        sample_map_for("original.ts")
    }

    fn data_url(map: &SourceMap) -> String {
        format!(
            "data:application/json;base64,{}",
            STANDARD.encode(map.to_json_string()),
        )
    }

    fn parse_code(code: &str) -> Ast {
        parse_ast(
            code.to_string(),
            ParseAstOptions {
                cwd: PathBuf::new(),
                file: String::from("index.ts"),
                language: None,
                source_type: None,
                input_map: None,
            },
        )
        .expect("fixture parses")
    }

    fn comments(ast: &Ast) -> Vec<String> {
        ast.program()
            .comments
            .iter()
            .map(|comment: &Comment| comment_text(ast, comment))
            .collect()
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
    fn test_last_non_empty_line_reports_crlf_start() {
        let url: String = data_url(&sample_map());
        let code: String =
            format!("const a = 1;\r\n//# sourceMappingURL={url}\r\n");

        let (line, start): (&str, usize) =
            last_non_empty_line(&code).expect("line present");

        assert_eq!(start, code.find("//#").expect("comment start"));
        assert!(line.starts_with("//# sourceMappingURL="));
        assert!(!line.ends_with('\r'));
    }

    #[test]
    fn test_strip_removes_consumed_inline_comment() {
        let url: String = data_url(&sample_map());
        let code: String =
            format!("const a = 1;\n//# sourceMappingURL={url}\n");
        let mut ast: Ast = parse_code(&code);

        assert!(ast.source().contains("sourceMappingURL"));
        assert_eq!(comments(&ast).len(), 1);

        let removed: bool = strip_consumed_inline_source_map_comment(
            &mut ast,
            &InputSourceMap::Mode(InputSourceMapMode::Inline),
        );

        assert!(removed);
        assert!(comments(&ast).is_empty());
        assert!(ast.source().contains("sourceMappingURL"));
    }

    #[test]
    fn test_strip_handles_slash_at_and_block_spellings() {
        let url: String = data_url(&sample_map());

        for code in [
            format!("const a = 1;\n//@ sourceMappingURL={url}\n"),
            format!("const a = 1;\n/*# sourceMappingURL={url} */\n"),
        ] {
            let mut ast: Ast = parse_code(&code);

            let removed: bool = strip_consumed_inline_source_map_comment(
                &mut ast,
                &InputSourceMap::Mode(InputSourceMapMode::Inline),
            );

            assert!(removed);
            assert!(comments(&ast).is_empty());
        }
    }

    #[test]
    fn test_strip_keeps_unconsumed_comments() {
        let url: String = data_url(&sample_map());
        let code: String = format!(
            "// see sourceMappingURL=docs\nconst a = 1;\n//# sourceMappingURL={url}\n",
        );
        let mut ast: Ast = parse_code(&code);

        let removed: bool = strip_consumed_inline_source_map_comment(
            &mut ast,
            &InputSourceMap::Mode(InputSourceMapMode::Inline),
        );

        assert!(removed);
        assert_eq!(
            comments(&ast),
            vec![String::from("// see sourceMappingURL=docs")],
        );
    }

    #[test]
    fn test_strip_keeps_earlier_inline_map_comment() {
        let first_url: String = data_url(&sample_map_for("first.ts"));
        let last_url: String = data_url(&sample_map_for("last.ts"));
        let code: String = format!(
            "//# sourceMappingURL={first_url}\nconst a = 1;\n//# sourceMappingURL={last_url}\n",
        );
        let mut ast: Ast = parse_code(&code);

        let removed: bool = strip_consumed_inline_source_map_comment(
            &mut ast,
            &InputSourceMap::Mode(InputSourceMapMode::Inline),
        );

        assert!(removed);
        assert_eq!(
            comments(&ast),
            vec![format!("//# sourceMappingURL={first_url}")],
        );
    }

    #[test]
    fn test_strip_ignores_value_and_external_modes() {
        let url: String = data_url(&sample_map());
        let code: String =
            format!("const a = 1;\n//# sourceMappingURL={url}\n");
        let mut ast: Ast = parse_code(&code);
        let before: Vec<String> = comments(&ast);

        for input in [
            InputSourceMap::Mode(InputSourceMapMode::Value(sample_map())),
            InputSourceMap::Mode(InputSourceMapMode::External),
        ] {
            let removed: bool =
                strip_consumed_inline_source_map_comment(&mut ast, &input);

            assert!(!removed);
        }

        assert_eq!(comments(&ast), before);
    }

    #[test]
    fn test_strip_handles_input_options_wrapper() {
        let url: String = data_url(&sample_map());
        let code: String =
            format!("const a = 1;\n//# sourceMappingURL={url}\n");
        let mut ast: Ast = parse_code(&code);
        let input: InputSourceMap =
            InputSourceMap::Options(InputSourceMapOptions {
                mode: InputSourceMapMode::Inline,
                path: None,
            });

        let removed: bool =
            strip_consumed_inline_source_map_comment(&mut ast, &input);

        assert!(removed);
        assert!(comments(&ast).is_empty());
    }

    #[test]
    fn test_strip_ignores_non_data_url_comment() {
        let code: &str = "const a = 1;\n//# sourceMappingURL=docs.map\n";
        let mut ast: Ast = parse_code(code);

        let removed: bool = strip_consumed_inline_source_map_comment(
            &mut ast,
            &InputSourceMap::Mode(InputSourceMapMode::Auto),
        );

        assert!(!removed);
        assert_eq!(
            comments(&ast),
            vec![String::from("//# sourceMappingURL=docs.map")],
        );
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
