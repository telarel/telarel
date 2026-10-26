use crate::_types::options::sourcemap::output::{
    OutputSourceMapMode, OutputSourceMapOptions,
};
use crate::ast::result::CodeResult;
use crate::helpers::path::external_map_url;
use crate::sourcemap::inline::{
    append_inline_source_map, append_source_mapping_url,
};

/// Apply the resolved output options to a [`CodeResult`].
///
/// `output_file` derives the default external map URL when `url` is absent.
/// Pass `None` when no output map was explicitly requested. The pipeline calls
/// this function after the winning generate hook, builtin or custom.
pub fn apply_output_source_map_mode(
    result: CodeResult,
    options: &OutputSourceMapOptions,
    output_file: Option<&str>,
) -> CodeResult {
    let CodeResult { code, map } = result;

    let Some(map) = map else {
        return CodeResult { code, map: None };
    };

    match options.mode {
        | OutputSourceMapMode::External => {
            let default_url: Option<String> =
                output_file.and_then(external_map_url);

            let url: Option<&str> = options
                .url
                .as_deref()
                .filter(|url: &&str| !url.is_empty())
                .or(default_url.as_deref());

            let code: String = match url {
                | Some(url) => append_source_mapping_url(code, url),
                | None => code,
            };

            CodeResult { code, map: Some(map) }
        },
        | OutputSourceMapMode::Inline => {
            CodeResult { code: append_inline_source_map(code, &map), map: None }
        },
        | OutputSourceMapMode::Both => CodeResult {
            code: append_inline_source_map(code, &map),
            map: Some(map),
        },
    }
}

#[cfg(test)]
mod tests {
    use oxc_sourcemap::SourceMapBuilder;

    use crate::sourcemap::SourceMap;
    use crate::sourcemap::inline::extract_inline_source_map;

    use super::*;

    fn sample_map() -> SourceMap {
        let mut builder: SourceMapBuilder<'_> = SourceMapBuilder::default();

        builder.set_file("original.ts");

        let source_id: u32 =
            builder.add_source_and_content("original.ts", "const a = 1;");

        builder.add_token(0, 0, 3, 2, Some(source_id), None);

        builder.into_owned_sourcemap().into_inner()
    }

    fn result_with_map() -> CodeResult {
        CodeResult {
            code: String::from("const a = 1;"),
            map: Some(sample_map()),
        }
    }

    fn result_without_map() -> CodeResult {
        CodeResult { code: String::from("const a = 1;"), map: None }
    }

    fn options(mode: OutputSourceMapMode) -> OutputSourceMapOptions {
        OutputSourceMapOptions { mode, url: None }
    }

    fn options_with_url(
        mode: OutputSourceMapMode,
        url: &str,
    ) -> OutputSourceMapOptions {
        OutputSourceMapOptions { mode, url: Some(url.to_owned()) }
    }

    #[test]
    fn test_external_without_output_file_returns_result_unchanged() {
        let result: CodeResult = apply_output_source_map_mode(
            result_with_map(),
            &options(OutputSourceMapMode::External),
            None,
        );

        assert_eq!(result.code, "const a = 1;");
        assert!(result.map.is_some());
    }

    #[test]
    fn test_external_appends_configured_url_and_keeps_map() {
        let result: CodeResult = apply_output_source_map_mode(
            result_with_map(),
            &options_with_url(OutputSourceMapMode::External, "dist/app.js.map"),
            None,
        );

        assert_eq!(
            result.code,
            "const a = 1;\n//# sourceMappingURL=dist/app.js.map\n",
        );
        assert!(result.map.is_some());
    }

    #[test]
    fn test_external_without_url_uses_output_file_map() {
        let result: CodeResult = apply_output_source_map_mode(
            result_with_map(),
            &options(OutputSourceMapMode::External),
            Some("dist/index.js"),
        );

        assert_eq!(
            result.code,
            "const a = 1;\n//# sourceMappingURL=dist/index.js.map\n",
        );
        assert!(result.map.is_some());
    }

    #[test]
    fn test_external_configured_url_wins_over_output_file() {
        let result: CodeResult = apply_output_source_map_mode(
            result_with_map(),
            &options_with_url(OutputSourceMapMode::External, "custom.js.map"),
            Some("dist/index.js"),
        );

        assert_eq!(
            result.code,
            "const a = 1;\n//# sourceMappingURL=custom.js.map\n",
        );
        assert!(result.map.is_some());
    }

    #[test]
    fn test_external_empty_url_falls_back_to_output_file() {
        let result: CodeResult = apply_output_source_map_mode(
            result_with_map(),
            &options_with_url(OutputSourceMapMode::External, ""),
            Some("dist/index.js"),
        );

        assert_eq!(
            result.code,
            "const a = 1;\n//# sourceMappingURL=dist/index.js.map\n",
        );
        assert!(result.map.is_some());
    }

    #[test]
    fn test_inline_appends_comment_and_drops_map() {
        let map: SourceMap = sample_map();

        let result: CodeResult = apply_output_source_map_mode(
            result_with_map(),
            &options(OutputSourceMapMode::Inline),
            Some("dist/index.js"),
        );

        assert!(result.map.is_none());
        assert!(result.code.starts_with("const a = 1;\n//# sourceMappingURL="));

        let extracted: SourceMap = extract_inline_source_map(&result.code)
            .expect("comment present")
            .expect("valid map");

        assert_eq!(extracted.to_json_string(), map.to_json_string());
    }

    #[test]
    fn test_inline_ignores_configured_url() {
        let result: CodeResult = apply_output_source_map_mode(
            result_with_map(),
            &options_with_url(OutputSourceMapMode::Inline, "dist/app.js.map"),
            Some("dist/index.js"),
        );

        assert!(result.map.is_none());
        assert!(
            result.code.starts_with("const a = 1;\n//# sourceMappingURL=data:")
        );
    }

    #[test]
    fn test_both_appends_comment_and_keeps_map() {
        let map: SourceMap = sample_map();

        let result: CodeResult = apply_output_source_map_mode(
            result_with_map(),
            &options(OutputSourceMapMode::Both),
            Some("dist/index.js"),
        );

        let kept: SourceMap = result.map.expect("map kept");

        assert_eq!(kept.to_json_string(), map.to_json_string());
        assert!(result.code.starts_with("const a = 1;\n//# sourceMappingURL="));
    }

    #[test]
    fn test_both_ignores_configured_url() {
        let result: CodeResult = apply_output_source_map_mode(
            result_with_map(),
            &options_with_url(OutputSourceMapMode::Both, "dist/app.js.map"),
            Some("dist/index.js"),
        );

        assert!(result.map.is_some());
        assert!(
            result.code.starts_with("const a = 1;\n//# sourceMappingURL=data:")
        );
    }

    #[test]
    fn test_missing_map_leaves_code_byte_identical_in_every_mode() {
        for mode in [
            OutputSourceMapMode::External,
            OutputSourceMapMode::Inline,
            OutputSourceMapMode::Both,
        ] {
            let result: CodeResult = apply_output_source_map_mode(
                result_without_map(),
                &options_with_url(mode, "dist/app.js.map"),
                Some("dist/index.js"),
            );

            assert_eq!(result.code, "const a = 1;");
            assert!(result.map.is_none());
        }
    }
}
