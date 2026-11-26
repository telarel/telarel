use std::path::Path;

use oxc::allocator::Allocator;
use oxc::ast::ast::{Comment, Program};

use crate::_types::options::sourcemap::input::{
    InputSourceMap, InputSourceMapMode,
};
use crate::_types::options::sourcemap::output::{
    OutputSourceMap, OutputSourceMapMode,
};
use crate::ast::ast::Ast;
use crate::ast::result::CodeResult;
use crate::errors::compile::CompileError;
use crate::helpers::path::external_map_url;
use crate::sourcemap::SourceMap;
use crate::sourcemap::inline::{
    append_inline_source_map, append_source_mapping_url,
    inline_source_map_marker,
};
use crate::sourcemap::resolve::resolve_input_source_map;

/// Remove the inline source-map comment that supplied `input`.
///
/// This function considers only `Inline` and `Auto` requests. It removes a
/// comment only when the last non-empty line carries a `sourceMappingURL`
/// comment with a base64 data URI. It removes the comment from
/// `Program.comments` and leaves `Ast::source` unchanged. This function
/// returns whether it removed a comment.
pub fn strip_consumed_inline_source_map_comment(
    ast: &mut Ast,
    input: &InputSourceMap,
) -> bool {
    let mode: &InputSourceMapMode = input.mode();

    if !matches!(mode, InputSourceMapMode::Inline | InputSourceMapMode::Auto) {
        return false;
    }

    let Some(marker): Option<usize> =
        inline_source_map_marker(ast.source().as_ref())
    else {
        return false;
    };

    ast.with_mut(|_allocator: &Allocator, program: &mut Program<'_>| -> bool {
        let before: usize = program.comments.len();

        program.comments.retain(|comment: &Comment| {
            !(comment.span.start as usize <= marker
                && marker < comment.span.end as usize)
        });

        program.comments.len() != before
    })
}

/// Attach the input map requested by `input` to `ast`.
///
/// An explicit request replaces the AST's own map when it resolves. A
/// best-effort miss (`Inline` or `Auto` with no map) and no request keep the
/// AST's own map. An `External` miss or corrupt map is an error from the
/// resolver.
pub fn attach_input_map(
    ast: &Ast,
    cwd: &Path,
    file: &str,
    input: Option<&InputSourceMap>,
) -> Result<Ast, CompileError> {
    let Some(input): Option<&InputSourceMap> = input else {
        return Ok(ast.clone());
    };

    let Some(map): Option<SourceMap> =
        resolve_input_source_map(ast.source(), cwd, file, input)?
    else {
        return Ok(ast.clone());
    };

    let mut mapped: Ast = ast.with_input_map(Some(map));

    strip_consumed_inline_source_map_comment(&mut mapped, input);

    Ok(mapped)
}

/// Apply the requested output source-map mode to a [`CodeResult`].
///
/// This function records a non-empty `output_file` in the map before it
/// applies the mode. `None` leaves the code unchanged. `output_file` also
/// derives the default external map URL for `External` requests without a URL.
/// The pipeline calls this function after the winning generate hook, builtin
/// or custom.
pub fn apply_output_source_map_mode(
    result: CodeResult,
    output: Option<&OutputSourceMap>,
    output_file: &str,
) -> CodeResult {
    let CodeResult { code, map } = result;

    let Some(mut map): Option<SourceMap> = map else {
        return CodeResult { code, map: None };
    };

    if !output_file.is_empty() {
        map.set_file(output_file);
    }

    let Some(output): Option<&OutputSourceMap> = output else {
        return CodeResult { code, map: Some(map) };
    };

    match output.mode() {
        | OutputSourceMapMode::External => {
            let default_url: Option<String> = external_map_url(output_file);

            let url: Option<&str> = output
                .url()
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
    use std::path::{Path, PathBuf};

    use base64::Engine;
    use base64::engine::general_purpose::STANDARD;
    use oxc_sourcemap::SourceMapBuilder;

    use crate::_types::options::sourcemap::input::InputSourceMapOptions;
    use crate::_types::options::sourcemap::output::OutputSourceMapOptions;
    use crate::ast::ast::Ast;
    use crate::ast::parse::{ParseAstOptions, parse_ast};
    use crate::helpers::test_util::comment_text;
    use crate::sourcemap::SourceMap;
    use crate::sourcemap::inline::{
        append_inline_source_map, extract_inline_source_map,
    };

    use super::*;

    fn strip_sample_map_for(file: &str) -> SourceMap {
        let mut builder: SourceMapBuilder<'_> = SourceMapBuilder::default();

        builder.set_file(file);

        let source_id: u32 =
            builder.add_source_and_content(file, "const a = 1;");

        builder.add_token(0, 0, 3, 2, Some(source_id), None);

        builder.into_owned_sourcemap().into_inner()
    }

    fn strip_sample_map() -> SourceMap {
        strip_sample_map_for("original.ts")
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

    fn attach_sample_map(file: &str) -> SourceMap {
        let mut builder: SourceMapBuilder<'_> = SourceMapBuilder::default();

        builder.set_file(file);

        let source_id: u32 =
            builder.add_source_and_content(file, "const a = 1;");

        builder.add_token(0, 0, 4, 0, Some(source_id), None);

        builder.into_owned_sourcemap().into_inner()
    }

    fn json_of(map: &SourceMap) -> String {
        map.to_json_string()
    }

    fn assert_same_map(
        actual: &SourceMap,
        expected: &SourceMap,
    ) {
        assert_eq!(json_of(actual), json_of(expected));
    }

    fn parse_ast_with_map(
        code: &str,
        map: Option<SourceMap>,
    ) -> Ast {
        parse_ast(
            code.to_string(),
            ParseAstOptions {
                cwd: PathBuf::new(),
                file: String::from("index.ts"),
                language: None,
                source_type: None,
                input_map: map,
            },
        )
        .expect("fixture parses")
    }

    fn output_sample_map() -> SourceMap {
        let mut builder: SourceMapBuilder<'_> = SourceMapBuilder::default();

        builder.set_file("original.ts");

        let source_id: u32 =
            builder.add_source_and_content("original.ts", "const a = 1;");

        builder.add_token(0, 0, 3, 2, Some(source_id), None);

        builder.into_owned_sourcemap().into_inner()
    }

    fn output_sample_map_with_file(file: &str) -> SourceMap {
        let mut map: SourceMap = output_sample_map();
        map.set_file(file);
        map
    }

    fn result_with_map() -> CodeResult {
        CodeResult {
            code: String::from("const a = 1;"),
            map: Some(output_sample_map()),
        }
    }

    fn result_without_map() -> CodeResult {
        CodeResult { code: String::from("const a = 1;"), map: None }
    }

    fn output(mode: OutputSourceMapMode) -> OutputSourceMap {
        OutputSourceMap::Mode(mode)
    }

    fn output_with_url(
        mode: OutputSourceMapMode,
        url: &str,
    ) -> OutputSourceMap {
        OutputSourceMap::Options(OutputSourceMapOptions {
            mode,
            url: Some(url.to_owned()),
        })
    }

    #[test]
    fn test_strip_removes_consumed_inline_comment() {
        let url: String = data_url(&strip_sample_map());
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
        let url: String = data_url(&strip_sample_map());

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
        let url: String = data_url(&strip_sample_map());
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
        let first_url: String = data_url(&strip_sample_map_for("first.ts"));
        let last_url: String = data_url(&strip_sample_map_for("last.ts"));
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
        let url: String = data_url(&strip_sample_map());
        let code: String =
            format!("const a = 1;\n//# sourceMappingURL={url}\n");
        let mut ast: Ast = parse_code(&code);
        let before: Vec<String> = comments(&ast);

        for input in [
            InputSourceMap::Mode(InputSourceMapMode::Value(strip_sample_map())),
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
        let url: String = data_url(&strip_sample_map());
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
    fn test_attach_value_request_replaces_ast_map() {
        let ast: Ast = parse_ast_with_map(
            "const a = 1;",
            Some(attach_sample_map("ast.ts")),
        );
        let replacement: SourceMap = attach_sample_map("replacement.ts");

        let mapped: Ast = attach_input_map(
            &ast,
            Path::new(""),
            "index.ts",
            Some(&InputSourceMap::Mode(InputSourceMapMode::Value(
                replacement.clone(),
            ))),
        )
        .expect("value request resolves");

        assert_same_map(mapped.map().expect("map attached"), &replacement);
    }

    #[test]
    fn test_attach_inline_request_replaces_ast_map_and_strips_comment() {
        let inline_map: SourceMap = attach_sample_map("inline.ts");
        let code: String =
            append_inline_source_map(String::from("const a = 1;"), &inline_map);
        let ast: Ast =
            parse_ast_with_map(&code, Some(attach_sample_map("ast.ts")));

        assert_eq!(ast.program().comments.len(), 1);

        let mapped: Ast = attach_input_map(
            &ast,
            Path::new(""),
            "index.ts",
            Some(&InputSourceMap::Mode(InputSourceMapMode::Inline)),
        )
        .expect("inline request resolves");

        assert_same_map(mapped.map().expect("map attached"), &inline_map);
        assert!(mapped.program().comments.is_empty());
        assert!(mapped.source().contains("sourceMappingURL"));
    }

    #[test]
    fn test_attach_best_effort_miss_keeps_ast_map() {
        let ast: Ast = parse_ast_with_map(
            "const a = 1;",
            Some(attach_sample_map("ast.ts")),
        );
        let expected: &SourceMap = ast.map().expect("fixture carries a map");

        for mode in [InputSourceMapMode::Inline, InputSourceMapMode::Auto] {
            let mapped: Ast = attach_input_map(
                &ast,
                Path::new(""),
                "index.ts",
                Some(&InputSourceMap::Mode(mode)),
            )
            .expect("best-effort miss is silent");

            assert_same_map(mapped.map().expect("map kept"), expected);
        }
    }

    #[test]
    fn test_attach_without_request_keeps_ast_map() {
        let ast: Ast = parse_ast_with_map(
            "const a = 1;",
            Some(attach_sample_map("ast.ts")),
        );
        let expected: &SourceMap = ast.map().expect("fixture carries a map");

        let mapped: Ast =
            attach_input_map(&ast, Path::new(""), "index.ts", None)
                .expect("no request never fails");

        assert_same_map(mapped.map().expect("map kept"), expected);
    }

    #[test]
    fn test_no_output_request_stamps_file_and_keeps_code() {
        let result: CodeResult = apply_output_source_map_mode(
            result_with_map(),
            None,
            "dist/index.js",
        );

        assert_eq!(result.code, "const a = 1;");

        let map: SourceMap = result.map.expect("map kept");

        assert_eq!(map.get_file(), Some("dist/index.js"));
        assert_eq!(
            map.to_json_string(),
            output_sample_map_with_file("dist/index.js").to_json_string(),
        );
    }

    #[test]
    fn test_external_empty_output_file_leaves_map_file_unchanged() {
        let result: CodeResult = apply_output_source_map_mode(
            result_with_map(),
            Some(&output(OutputSourceMapMode::External)),
            "",
        );

        assert_eq!(result.code, "const a = 1;");

        let map: SourceMap = result.map.expect("map kept");

        assert_eq!(map.get_file(), Some("original.ts"));
        assert_eq!(map.to_json_string(), output_sample_map().to_json_string());
    }

    #[test]
    fn test_external_appends_configured_url_and_keeps_map() {
        let result: CodeResult = apply_output_source_map_mode(
            result_with_map(),
            Some(&output_with_url(
                OutputSourceMapMode::External,
                "dist/app.js.map",
            )),
            "",
        );

        assert_eq!(
            result.code,
            "const a = 1;\n//# sourceMappingURL=dist/app.js.map\n",
        );

        let map: SourceMap = result.map.expect("map kept");

        assert_eq!(map.get_file(), Some("original.ts"));
    }

    #[test]
    fn test_external_without_url_uses_output_file_map() {
        let result: CodeResult = apply_output_source_map_mode(
            result_with_map(),
            Some(&output(OutputSourceMapMode::External)),
            "dist/index.js",
        );

        assert_eq!(
            result.code,
            "const a = 1;\n//# sourceMappingURL=dist/index.js.map\n",
        );

        let map: SourceMap = result.map.expect("map kept");

        assert_eq!(map.get_file(), Some("dist/index.js"));
    }

    #[test]
    fn test_external_configured_url_wins_over_output_file() {
        let result: CodeResult = apply_output_source_map_mode(
            result_with_map(),
            Some(&output_with_url(
                OutputSourceMapMode::External,
                "custom.js.map",
            )),
            "dist/index.js",
        );

        assert_eq!(
            result.code,
            "const a = 1;\n//# sourceMappingURL=custom.js.map\n",
        );

        let map: SourceMap = result.map.expect("map kept");

        assert_eq!(map.get_file(), Some("dist/index.js"));
    }

    #[test]
    fn test_external_empty_url_falls_back_to_output_file() {
        let result: CodeResult = apply_output_source_map_mode(
            result_with_map(),
            Some(&output_with_url(OutputSourceMapMode::External, "")),
            "dist/index.js",
        );

        assert_eq!(
            result.code,
            "const a = 1;\n//# sourceMappingURL=dist/index.js.map\n",
        );

        let map: SourceMap = result.map.expect("map kept");

        assert_eq!(map.get_file(), Some("dist/index.js"));
    }

    #[test]
    fn test_inline_appends_comment_and_drops_map() {
        let map: SourceMap = output_sample_map_with_file("dist/index.js");

        let result: CodeResult = apply_output_source_map_mode(
            result_with_map(),
            Some(&output(OutputSourceMapMode::Inline)),
            "dist/index.js",
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
            Some(&output_with_url(
                OutputSourceMapMode::Inline,
                "dist/app.js.map",
            )),
            "dist/index.js",
        );

        assert!(result.map.is_none());
        assert!(
            result.code.starts_with("const a = 1;\n//# sourceMappingURL=data:")
        );
    }

    #[test]
    fn test_both_appends_comment_and_keeps_map() {
        let map: SourceMap = output_sample_map_with_file("dist/index.js");

        let result: CodeResult = apply_output_source_map_mode(
            result_with_map(),
            Some(&output(OutputSourceMapMode::Both)),
            "dist/index.js",
        );

        let kept: SourceMap = result.map.expect("map kept");

        assert_eq!(kept.to_json_string(), map.to_json_string());
        assert!(result.code.starts_with("const a = 1;\n//# sourceMappingURL="));
    }

    #[test]
    fn test_both_ignores_configured_url() {
        let result: CodeResult = apply_output_source_map_mode(
            result_with_map(),
            Some(&output_with_url(
                OutputSourceMapMode::Both,
                "dist/app.js.map",
            )),
            "dist/index.js",
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
                Some(&output_with_url(mode, "dist/app.js.map")),
                "dist/index.js",
            );

            assert_eq!(result.code, "const a = 1;");
            assert!(result.map.is_none());
        }
    }
}
