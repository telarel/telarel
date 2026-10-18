use crate::_types::options::source_map::output::OutputSourceMapMode;
use crate::ast::result::CodeResult;
use crate::sourcemap::inline::append_inline_source_map;

/// Apply the resolved output mode to a [`CodeResult`].
///
/// Called by the pipeline after the winning generate hook (builtin or custom).
pub fn apply_output_source_map_mode(
    result: CodeResult,
    mode: OutputSourceMapMode,
) -> CodeResult {
    let CodeResult { code, map } = result;

    let Some(map) = map else {
        return CodeResult { code, map: None };
    };

    match mode {
        | OutputSourceMapMode::External => CodeResult { code, map: Some(map) },
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

    #[test]
    fn test_external_returns_result_unchanged() {
        let result: CodeResult = apply_output_source_map_mode(
            result_with_map(),
            OutputSourceMapMode::External,
        );

        assert_eq!(result.code, "const a = 1;");
        assert!(result.map.is_some());
    }

    #[test]
    fn test_inline_appends_comment_and_drops_map() {
        let map: SourceMap = sample_map();

        let result: CodeResult = apply_output_source_map_mode(
            result_with_map(),
            OutputSourceMapMode::Inline,
        );

        assert!(result.map.is_none());
        assert!(result.code.starts_with("const a = 1;\n//# sourceMappingURL="));

        let extracted: SourceMap = extract_inline_source_map(&result.code)
            .expect("comment present")
            .expect("valid map");

        assert_eq!(extracted.to_json_string(), map.to_json_string());
    }

    #[test]
    fn test_both_appends_comment_and_keeps_map() {
        let map: SourceMap = sample_map();

        let result: CodeResult = apply_output_source_map_mode(
            result_with_map(),
            OutputSourceMapMode::Both,
        );

        let kept: SourceMap = result.map.expect("map kept");

        assert_eq!(kept.to_json_string(), map.to_json_string());
        assert!(result.code.starts_with("const a = 1;\n//# sourceMappingURL="));
    }

    #[test]
    fn test_missing_map_leaves_code_byte_identical_in_every_mode() {
        for mode in [
            OutputSourceMapMode::External,
            OutputSourceMapMode::Inline,
            OutputSourceMapMode::Both,
        ] {
            let result: CodeResult =
                apply_output_source_map_mode(result_without_map(), mode);

            assert_eq!(result.code, "const a = 1;");
            assert!(result.map.is_none());
        }
    }
}
