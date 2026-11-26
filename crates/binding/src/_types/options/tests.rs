use telarel_common::{
    InputSourceMapMode, InputSourceMapOptions, Language, OutputOptions,
    OutputSourceMap, OutputSourceMapMode, OutputSourceMapOptions,
    ResolvedOptions, SourceType,
};

use super::*;

fn options() -> JsParseOptions {
    JsParseOptions {
        cwd: None,
        file: Some(String::from("index.js")),
        language: None,
        source_type: None,
        input_source_map: None,
        parser: None,
        plugins: Vec::new(),
    }
}

#[test]
fn test_parse_options_language_labels_roundtrip() {
    let expected: [(&str, Language); 5] = [
        ("js", Language::JS),
        ("ts", Language::TS),
        ("dts", Language::DTS),
        ("jsx", Language::JSX),
        ("tsx", Language::TSX),
    ];

    for (label, language) in expected {
        let mut options: JsParseOptions = options();

        options.language = Some(String::from(label));

        let parsed: ParseOptions =
            options.to_parse_options().expect("valid options parse");

        assert_eq!(parsed.language, Some(language), "{label}");
    }
}

#[test]
fn test_parse_source_type_valid_values() {
    let expected: [(&str, SourceType); 4] = [
        ("script", SourceType::Script),
        ("commonjs", SourceType::CommonJS),
        ("module", SourceType::Module),
        ("unambiguous", SourceType::Unambiguous),
    ];

    for (text, source_type) in expected {
        assert_eq!(parse_source_type(text), Ok(source_type), "{text}");
    }
}

#[test]
fn test_parse_source_type_invalid_value_errors() {
    for text in ["", "js", "ts", "Module", "commonJS"] {
        assert!(parse_source_type(text).is_err(), "{text} must error");
    }
}

#[test]
fn test_parse_options_maps_valid_values() {
    let mut options: JsParseOptions = options();

    options.language = Some(String::from("tsx"));
    options.source_type = Some(String::from("commonjs"));

    let parsed: ParseOptions =
        options.to_parse_options().expect("valid options parse");

    assert_eq!(parsed.language, Some(Language::TSX));
    assert_eq!(parsed.source_type, Some(SourceType::CommonJS));
    assert_eq!(parsed.file.as_deref(), Some("index.js"));
    assert_eq!(parsed.cwd, None);
    assert!(parsed.input_source_map.is_none());
}

#[test]
fn test_parse_options_keeps_missing_options_none() {
    let parsed: ParseOptions =
        options().to_parse_options().expect("absent options parse");

    assert_eq!(parsed.language, None);
    assert_eq!(parsed.source_type, None);
    assert!(parsed.input_source_map.is_none());
}

#[test]
fn test_pipeline_options_share_the_same_scalars() {
    let parse: ParseOptions =
        options().to_parse_options().expect("parse options parse");

    let compile: CompileOptions = JsCompileOptions {
        cwd: None,
        file: Some(String::from("index.js")),
        language: None,
        source_type: None,
        output: None,
        source_map: None,
        parser: None,
        generator: None,
        plugins: Vec::new(),
    }
    .to_compile_options()
    .expect("compile options parse");

    let rewrite: RewriteOptions = JsRewriteOptions {
        cwd: None,
        file: Some(String::from("index.js")),
        language: None,
        source_type: None,
        input_source_map: None,
        plugins: Vec::new(),
    }
    .to_rewrite_options()
    .expect("rewrite options parse");

    let from_ast: CompileFromAstOptions = JsCompileFromAstOptions {
        cwd: None,
        file: Some(String::from("index.js")),
        language: None,
        source_type: None,
        output: None,
        source_map: None,
        generator: None,
        plugins: Vec::new(),
    }
    .to_compile_from_ast_options()
    .expect("compile-from-ast options parse");

    assert_eq!(parse.file.as_deref(), Some("index.js"));
    assert_eq!(compile.file.as_deref(), Some("index.js"));
    assert_eq!(rewrite.file.as_deref(), Some("index.js"));
    assert_eq!(from_ast.file.as_deref(), Some("index.js"));
}

#[test]
fn test_parse_options_invalid_language_errors() {
    let mut options: JsParseOptions = options();

    options.language = Some(String::from("json"));

    let error: napi::Error =
        options.to_parse_options().expect_err("invalid language errors");

    assert_eq!(error.status, napi::Status::InvalidArg);
    assert!(error.reason.contains("invalid language \"json\""));
}

#[test]
fn test_parse_options_invalid_source_type_errors() {
    let mut options: JsParseOptions = options();

    options.source_type = Some(String::from("js"));

    let error: napi::Error =
        options.to_parse_options().expect_err("invalid source type errors");

    assert_eq!(error.status, napi::Status::InvalidArg);
    assert!(error.reason.contains("invalid source type \"js\""));
}

fn options_with_input_source_map(value: serde_json::Value) -> JsParseOptions {
    let mut options: JsParseOptions = options();

    options.input_source_map = Some(value);

    options
}

#[test]
fn test_input_source_map_omitted_is_off() {
    let parsed: ParseOptions =
        options().to_parse_options().expect("absent inputSourceMap parses");

    assert!(parsed.input_source_map.is_none());
}

#[test]
fn test_input_source_map_mode_labels() {
    for (label, expected) in [
        ("inline", InputSourceMapMode::Inline),
        ("external", InputSourceMapMode::External),
        ("auto", InputSourceMapMode::Auto),
    ] {
        let parsed: ParseOptions =
            options_with_input_source_map(serde_json::json!(label))
                .to_parse_options()
                .expect("input label parses");

        let input: InputSourceMap =
            parsed.input_source_map.expect("input map is present");

        match input {
            | InputSourceMap::Mode(mode) => {
                assert!(
                    std::mem::discriminant(&mode)
                        == std::mem::discriminant(&expected),
                    "{label} maps to {mode:?}"
                );
            },
            | InputSourceMap::Options(options) => {
                panic!("expected a bare mode, got {options:?}");
            },
        }
    }
}

#[test]
fn test_input_source_map_path_string_is_invalid() {
    let error: napi::Error =
        options_with_input_source_map(serde_json::json!("maps/app.js.map"))
            .to_parse_options()
            .expect_err("path strings are rejected");

    assert_eq!(error.status, napi::Status::InvalidArg);

    assert!(error.reason.contains("invalid inputSourceMap"), "{error}",);
}

#[test]
fn test_input_source_map_options_with_external_path() {
    let parsed: ParseOptions =
        options_with_input_source_map(serde_json::json!({
            "mode": "external",
            "path": "maps/app.js.map",
        }))
        .to_parse_options()
        .expect("input options parse");

    let input: InputSourceMap =
        parsed.input_source_map.expect("input map is present");

    match input {
        | InputSourceMap::Options(options) => {
            assert!(matches!(options.mode, InputSourceMapMode::External,));

            assert_eq!(
                options.path.as_deref(),
                Some(std::path::Path::new("maps/app.js.map")),
            );
        },
        | other => panic!("expected an options input, got {other:?}"),
    }
}

#[test]
fn test_input_source_map_options_ignore_path_for_inline() {
    let parsed: ParseOptions = options_with_input_source_map(
        serde_json::json!({ "mode": "inline", "path": "maps/app.js.map" }),
    )
    .to_parse_options()
    .expect("inline options parse");

    let input: InputSourceMap =
        parsed.input_source_map.expect("input map is present");

    assert!(matches!(
        input,
        InputSourceMap::Options(InputSourceMapOptions {
            mode: InputSourceMapMode::Inline,
            path: Some(_),
        })
    ));
}

#[test]
fn test_input_source_map_options_require_mode() {
    let error: napi::Error =
        options_with_input_source_map(serde_json::json!({ "path": "a.map" }))
            .to_parse_options()
            .expect_err("missing mode errors");

    assert_eq!(error.status, napi::Status::InvalidArg);
    assert!(error.reason.contains("requires a `mode`"), "{error}");
}

#[test]
fn test_input_source_map_options_reject_invalid_path() {
    for path in [serde_json::json!(42), serde_json::json!("")] {
        let error: napi::Error = options_with_input_source_map(
            serde_json::json!({ "mode": "external", "path": path }),
        )
        .to_parse_options()
        .expect_err("invalid path errors");

        assert_eq!(error.status, napi::Status::InvalidArg);
        assert!(error.reason.contains("invalid inputSourceMap.path"));
    }
}

#[test]
fn test_input_source_map_object_is_value() {
    let parsed: ParseOptions =
        options_with_input_source_map(serde_json::json!({
            "version": 3,
            "sources": ["upstream.ts"],
            "mappings": "AAAA",
        }))
        .to_parse_options()
        .expect("object input parses");

    let input: InputSourceMap =
        parsed.input_source_map.expect("input map is present");

    match input {
        | InputSourceMap::Mode(InputSourceMapMode::Value(map)) => {
            let json: oxc_sourcemap::JSONSourceMap = map.to_json();

            assert_eq!(json.sources, vec![Some(String::from("upstream.ts"))],);
        },
        | other => panic!("expected a value input, got {other:?}"),
    }
}

#[test]
fn test_input_source_map_options_accept_value_mode() {
    let parsed: ParseOptions =
        options_with_input_source_map(serde_json::json!({
            "mode": {
                "version": 3,
                "sources": ["upstream.ts"],
                "mappings": "AASA",
            },
            "path": "ignored.map",
        }))
        .to_parse_options()
        .expect("value options parse");

    let input: InputSourceMap =
        parsed.input_source_map.expect("input map is present");

    match input {
        | InputSourceMap::Options(InputSourceMapOptions {
            mode: InputSourceMapMode::Value(map),
            path,
        }) => {
            let json: oxc_sourcemap::JSONSourceMap = map.to_json();

            assert_eq!(json.sources, vec![Some(String::from("upstream.ts"))],);

            assert_eq!(
                path.as_deref(),
                Some(std::path::Path::new("ignored.map")),
            );
        },
        | other => {
            panic!("expected options with a value mode, got {other:?}")
        },
    }
}

#[test]
fn test_input_source_map_options_reject_invalid_mode() {
    for mode in [
        serde_json::json!(42),
        serde_json::json!(null),
        serde_json::json!(true),
    ] {
        let error: napi::Error =
            options_with_input_source_map(serde_json::json!({ "mode": mode }))
                .to_parse_options()
                .expect_err("invalid mode errors");

        assert_eq!(error.status, napi::Status::InvalidArg);
        assert!(
            error.reason.contains("invalid inputSourceMap.mode"),
            "{error}",
        );
    }
}

#[test]
fn test_input_source_map_options_reject_invalid_value_mode() {
    let error: napi::Error = options_with_input_source_map(
        serde_json::json!({ "mode": { "version": 3 } }),
    )
    .to_parse_options()
    .expect_err("invalid value mode errors");

    assert_eq!(error.status, napi::Status::InvalidArg);
    assert!(
        error.reason.contains("invalid inputSourceMap.mode object"),
        "{error}",
    );
}

#[test]
fn test_input_source_map_invalid_shape_errors() {
    for value in
        [serde_json::json!(42), serde_json::json!({ "input": "inline" })]
    {
        let error: napi::Error = options_with_input_source_map(value)
            .to_parse_options()
            .expect_err("invalid inputSourceMap errors");

        assert_eq!(error.status, napi::Status::InvalidArg);
        assert!(error.reason.contains("invalid inputSourceMap"));
    }
}

#[test]
fn test_rewrite_options_parse_input_source_map() {
    let options: JsRewriteOptions = JsRewriteOptions {
        cwd: None,
        file: Some(String::from("index.js")),
        language: None,
        source_type: None,
        input_source_map: Some(serde_json::json!("inline")),
        plugins: Vec::new(),
    };

    let parsed: RewriteOptions =
        options.to_rewrite_options().expect("rewrite inputSourceMap parses");

    assert!(matches!(
        parsed.input_source_map,
        Some(InputSourceMap::Mode(InputSourceMapMode::Inline)),
    ));
}

#[test]
fn test_generate_options_forward_language_and_drop_source_type() {
    let options: JsGenerateOptions = JsGenerateOptions {
        cwd: Some(String::from("/repo")),
        file: Some(String::from("out.js")),
        language: Some(String::from("tsx")),
        output: None,
        source_map: Some(serde_json::json!({ "output": "inline" })),
        generator: None,
        plugins: Vec::new(),
    };

    let parsed: GenerateOptions =
        options.to_generate_options().expect("generate options parse");

    assert_eq!(parsed.cwd, Some(PathBuf::from("/repo")));
    assert_eq!(parsed.file.as_deref(), Some("out.js"));
    assert_eq!(parsed.language, Some(Language::TSX));
    assert!(parsed.source_map.is_some());
}

#[test]
fn test_generate_options_absent_language_stays_unset() {
    let options: JsGenerateOptions = JsGenerateOptions {
        cwd: None,
        file: None,
        language: None,
        output: None,
        source_map: None,
        generator: None,
        plugins: Vec::new(),
    };

    let parsed: GenerateOptions =
        options.to_generate_options().expect("generate options parse");

    assert_eq!(parsed.language, None);
}

#[test]
fn test_generate_options_invalid_language_errors() {
    let options: JsGenerateOptions = JsGenerateOptions {
        cwd: None,
        file: None,
        language: Some(String::from("json")),
        output: None,
        source_map: None,
        generator: None,
        plugins: Vec::new(),
    };

    let error: napi::Error =
        options.to_generate_options().expect_err("invalid language errors");

    assert_eq!(error.status, napi::Status::InvalidArg);
    assert!(error.reason.contains("invalid language \"json\""));
}

fn compile_options_with_source_map(
    value: serde_json::Value
) -> JsCompileOptions {
    JsCompileOptions {
        cwd: None,
        file: Some(String::from("index.js")),
        language: None,
        source_type: None,
        output: None,
        source_map: Some(value),
        parser: None,
        generator: None,
        plugins: Vec::new(),
    }
}

#[test]
fn test_compile_options_source_map_empty_object_is_off() {
    let parsed: CompileOptions =
        compile_options_with_source_map(serde_json::json!({}))
            .to_compile_options()
            .expect("empty sourceMap parses");

    let source_map: SourceMapOptions =
        parsed.source_map.expect("sourceMap is present");

    assert!(source_map.input.is_none());
    assert!(source_map.output.is_none());
}

#[test]
fn test_compile_options_source_map_input_invalid_shape_errors() {
    let error: napi::Error =
        compile_options_with_source_map(serde_json::json!({ "input": 42 }))
            .to_compile_options()
            .expect_err("invalid input shape errors");

    assert_eq!(error.status, napi::Status::InvalidArg);
    assert!(error.reason.contains("invalid sourceMap.input"));
}

#[test]
fn test_compile_options_source_map_output_mode_string() {
    let parsed: CompileOptions = compile_options_with_source_map(
        serde_json::json!({ "output": "both" }),
    )
    .to_compile_options()
    .expect("output label parses");

    assert!(matches!(
        parsed.source_map.expect("sourceMap is present").output,
        Some(OutputSourceMap::Mode(OutputSourceMapMode::Both))
    ));
}

#[test]
fn test_compile_options_source_map_output_options_object() {
    let parsed: CompileOptions = compile_options_with_source_map(
        serde_json::json!({ "output": { "mode": "inline" } }),
    )
    .to_compile_options()
    .expect("output options parse");

    assert!(matches!(
        parsed.source_map.expect("sourceMap is present").output,
        Some(OutputSourceMap::Options(OutputSourceMapOptions {
            mode: OutputSourceMapMode::Inline,
            url: None,
        }))
    ));
}

#[test]
fn test_compile_options_output_file() {
    let options: CompileOptions = JsCompileOptions {
        cwd: None,
        file: Some(String::from("src/index.ts")),
        language: None,
        source_type: None,
        output: Some(JsOutputOptions {
            file: Some(String::from("dist/index.js")),
        }),
        source_map: None,
        parser: None,
        generator: None,
        plugins: Vec::new(),
    }
    .to_compile_options()
    .expect("output options parse");

    let output: OutputOptions = options.output.expect("output is carried");

    assert_eq!(output.file.as_deref(), Some("dist/index.js"));
}

#[test]
fn test_compile_options_empty_output_file_is_unset() {
    let options: CompileOptions = JsCompileOptions {
        cwd: None,
        file: Some(String::from("src/index.ts")),
        language: None,
        source_type: None,
        output: Some(JsOutputOptions { file: Some(String::new()) }),
        source_map: None,
        parser: None,
        generator: None,
        plugins: Vec::new(),
    }
    .to_compile_options()
    .expect("empty output file is unset");

    let output: OutputOptions = options.output.expect("output is carried");

    assert!(output.file.is_none());
}

#[test]
fn test_compile_from_ast_options_output_file() {
    let options: CompileFromAstOptions = JsCompileFromAstOptions {
        cwd: None,
        file: Some(String::from("src/index.ts")),
        language: None,
        source_type: None,
        output: Some(JsOutputOptions {
            file: Some(String::from("dist/index.js")),
        }),
        source_map: None,
        generator: None,
        plugins: Vec::new(),
    }
    .to_compile_from_ast_options()
    .expect("output options parse");

    let output: OutputOptions = options.output.expect("output is carried");

    assert_eq!(output.file.as_deref(), Some("dist/index.js"));
}

#[test]
fn test_generate_options_output_file() {
    let options: GenerateOptions = JsGenerateOptions {
        cwd: None,
        file: Some(String::from("src/index.ts")),
        language: None,
        output: Some(JsOutputOptions {
            file: Some(String::from("dist/index.js")),
        }),
        source_map: None,
        generator: None,
        plugins: Vec::new(),
    }
    .to_generate_options()
    .expect("output options parse");

    let output: OutputOptions = options.output.expect("output is carried");

    assert_eq!(output.file.as_deref(), Some("dist/index.js"));
}

#[test]
fn test_compile_options_source_map_output_options_with_url() {
    let parsed: CompileOptions =
        compile_options_with_source_map(serde_json::json!({
            "output": {
                "mode": "external",
                "url": "dist/app.js.map",
            },
        }))
        .to_compile_options()
        .expect("output options with url parse");

    assert!(matches!(
        parsed.source_map.expect("sourceMap is present").output,
        Some(OutputSourceMap::Options(OutputSourceMapOptions {
            mode: OutputSourceMapMode::External,
            url: Some(url),
        })) if url == "dist/app.js.map"
    ));
}

#[test]
fn test_compile_options_source_map_output_options_reject_non_string_url() {
    let error: napi::Error =
        compile_options_with_source_map(serde_json::json!({
            "output": { "mode": "external", "url": 42 },
        }))
        .to_compile_options()
        .expect_err("non-string output url errors");

    assert_eq!(error.status, napi::Status::InvalidArg);
    assert!(error.reason.contains("invalid sourceMap.output.url"));
}

#[test]
fn test_compile_options_source_map_output_options_empty_url_is_unset() {
    let parsed: CompileOptions =
        compile_options_with_source_map(serde_json::json!({
            "output": { "mode": "external", "url": "" },
        }))
        .to_compile_options()
        .expect("empty output url is unset");

    assert!(matches!(
        parsed.source_map.expect("sourceMap is present").output,
        Some(OutputSourceMap::Options(OutputSourceMapOptions {
            mode: OutputSourceMapMode::External,
            url: None,
        }))
    ));
}

#[test]
fn test_compile_options_source_map_output_invalid_mode_errors() {
    let error: napi::Error = compile_options_with_source_map(
        serde_json::json!({ "output": "sideways" }),
    )
    .to_compile_options()
    .expect_err("invalid output mode errors");

    assert_eq!(error.status, napi::Status::InvalidArg);
    assert!(error.reason.contains("invalid sourceMap.output mode"));
}

#[test]
fn test_compile_options_source_map_output_options_without_mode_errors() {
    let error: napi::Error =
        compile_options_with_source_map(serde_json::json!({ "output": {} }))
            .to_compile_options()
            .expect_err("missing mode errors");

    assert_eq!(error.status, napi::Status::InvalidArg);
    assert!(error.reason.contains("requires a `mode`"));
}

#[test]
fn test_compile_options_source_map_not_an_object_errors() {
    let error: napi::Error =
        compile_options_with_source_map(serde_json::json!("inline"))
            .to_compile_options()
            .expect_err("non-object sourceMap errors");

    assert_eq!(error.status, napi::Status::InvalidArg);
    assert!(error.reason.contains("invalid sourceMap"));
}

#[test]
fn test_compile_options_cwd_becomes_path_buf() {
    let options: JsCompileOptions = JsCompileOptions {
        cwd: Some(String::from("/repo")),
        file: Some(String::from("index.js")),
        language: None,
        source_type: None,
        output: None,
        source_map: None,
        parser: None,
        generator: None,
        plugins: Vec::new(),
    };

    let parsed: CompileOptions =
        options.to_compile_options().expect("compile options parse");

    assert_eq!(parsed.cwd, Some(PathBuf::from("/repo")));
    assert!(parsed.source_map.is_none());
}

#[test]
fn test_resolved_parser_options_convert_to_resolvable() {
    let options: JsResolvedParserOptions = JsResolvedParserOptions {
        cwd: Some(String::from("/repo")),
        file: Some(String::from("app.tsx")),
        language: Some(String::from("tsx")),
        source_type: Some(String::from("module")),
        source_map: None,
        preserve_parens: Some(true),
        allow_return_outside_function: Some(true),
        oxc: Some(serde_json::json!({ "enableIdentHashes": false })),
    };

    let resolvable: ResolvableOptions =
        options.to_resolvable().expect("parser options parse");

    assert_eq!(resolvable.cwd, Some(PathBuf::from("/repo")));
    assert_eq!(resolvable.file.as_deref(), Some("app.tsx"));
    assert_eq!(resolvable.language, Some(Language::TSX));
    assert_eq!(resolvable.source_type, Some(SourceType::Module));
    assert!(resolvable.source_map.is_none());

    let parser: ParserOptions =
        resolvable.parser.expect("the parser group is carried");
    assert_eq!(parser.preserve_parens, Some(true));
    assert_eq!(parser.allow_return_outside_function, Some(true));
    assert_eq!(
        parser.oxc.expect("raw oxc is carried").enable_ident_hashes,
        Some(false)
    );
}

#[test]
fn test_resolved_generator_options_convert_to_resolvable_without_grammar() {
    let options: JsResolvedGeneratorOptions = JsResolvedGeneratorOptions {
        cwd: Some(String::from("/repo")),
        file: Some(String::from("out.js")),
        source_map: None,
        single_quote: Some(true),
        ascii_only: Some(true),
        indent_style: Some(String::from("tab")),
        indent_width: Some(4),
        comments: Some(false),
        minify: Some(true),
        oxc: Some(serde_json::json!({ "initialIndent": 1 })),
    };

    let resolvable: ResolvableOptions =
        options.to_resolvable().expect("generator options parse");

    assert_eq!(resolvable.cwd, Some(PathBuf::from("/repo")));
    assert_eq!(resolvable.file.as_deref(), Some("out.js"));
    assert_eq!(resolvable.language, None);
    assert_eq!(resolvable.source_type, None);

    let generator: GeneratorOptions =
        resolvable.generator.expect("the generator group is carried");
    assert_eq!(generator.single_quote, Some(true));
    assert_eq!(generator.ascii_only, Some(true));
    assert_eq!(generator.indent_style, Some(IndentStyle::Tab));
    assert_eq!(generator.indent_width, Some(4));
    assert_eq!(generator.comments, Some(false));
    assert_eq!(generator.minify, Some(true));
    assert_eq!(
        generator.oxc.expect("raw oxc is carried").initial_indent,
        Some(1)
    );
}

#[test]
fn test_parse_options_parse_parser_group() {
    let mut options: JsParseOptions = options();

    options.parser = Some(JsParserOptions {
        preserve_parens: Some(true),
        allow_return_outside_function: Some(true),
        oxc: Some(serde_json::json!({
            "parseRegularExpression": true,
            "enableIdentHashes": false,
        })),
    });

    let parsed: ParseOptions =
        options.to_parse_options().expect("parser group parses");

    let parser: ParserOptions = parsed.parser.expect("parser group present");

    assert_eq!(parser.preserve_parens, Some(true));
    assert_eq!(parser.allow_return_outside_function, Some(true));

    let oxc: PartialOxcParseOptions = parser.oxc.expect("raw oxc present");

    assert_eq!(oxc.parse_regular_expression, Some(true));
    assert_eq!(oxc.enable_ident_hashes, Some(false));
    assert_eq!(oxc.preserve_parens, None);
}

#[test]
fn test_parse_options_raw_partial_keeps_unset_curated_field_default() {
    let mut options: JsParseOptions = options();

    options.parser = Some(JsParserOptions {
        preserve_parens: None,
        allow_return_outside_function: None,
        oxc: Some(serde_json::json!({ "parseRegularExpression": true })),
    });

    let parsed: ParseOptions =
        options.to_parse_options().expect("raw partial parses");

    let parser: &ParserOptions =
        parsed.parser.as_ref().expect("parser group present");

    assert_eq!(parser.preserve_parens, None);
    assert_eq!(
        parser.oxc.as_ref().expect("raw oxc present").preserve_parens,
        None
    );

    let resolved: ResolvedOptions = telarel_common::resolve_options(
        ResolvableOptions::from(parsed),
        vec![],
    );

    assert!(!resolved.parser.preserve_parens);
    assert!(!resolved.parser.oxc.preserve_parens);
    assert!(resolved.parser.oxc.parse_regular_expression);
}

#[test]
fn test_compile_options_parse_parser_and_generator_groups() {
    let options: JsCompileOptions = JsCompileOptions {
        cwd: None,
        file: Some(String::from("index.ts")),
        language: None,
        source_type: None,
        output: None,
        source_map: None,
        parser: Some(JsParserOptions {
            preserve_parens: Some(true),
            ..JsParserOptions::default()
        }),
        generator: Some(JsGeneratorOptions {
            single_quote: Some(true),
            ..JsGeneratorOptions::default()
        }),
        plugins: Vec::new(),
    };

    let parsed: CompileOptions =
        options.to_compile_options().expect("groups parse");

    assert_eq!(
        parsed.parser.expect("parser group").preserve_parens,
        Some(true)
    );
    assert_eq!(
        parsed.generator.expect("generator group").single_quote,
        Some(true)
    );
}

#[test]
fn test_generate_options_parse_generator_group() {
    let options: JsGenerateOptions = JsGenerateOptions {
        cwd: None,
        file: Some(String::from("out.js")),
        language: None,
        output: None,
        source_map: None,
        generator: Some(JsGeneratorOptions {
            single_quote: Some(true),
            indent_style: Some(String::from("tab")),
            ..JsGeneratorOptions::default()
        }),
        plugins: Vec::new(),
    };

    let parsed: GenerateOptions =
        options.to_generate_options().expect("generator group parses");

    let generator: GeneratorOptions =
        parsed.generator.expect("generator group present");

    assert_eq!(generator.single_quote, Some(true));
    assert_eq!(generator.indent_style, Some(IndentStyle::Tab));
}

#[test]
fn test_compile_from_ast_options_parse_generator_group() {
    let options: JsCompileFromAstOptions = JsCompileFromAstOptions {
        cwd: None,
        file: Some(String::from("out.js")),
        language: None,
        source_type: None,
        output: None,
        source_map: None,
        generator: Some(JsGeneratorOptions {
            ascii_only: Some(true),
            ..JsGeneratorOptions::default()
        }),
        plugins: Vec::new(),
    };

    let parsed: CompileFromAstOptions =
        options.to_compile_from_ast_options().expect("generator group parses");

    assert_eq!(
        parsed.generator.expect("generator group").ascii_only,
        Some(true)
    );
}

#[test]
fn test_parser_options_raw_oxc_accepts_camel_case_fields() {
    let options: JsParserOptions = JsParserOptions {
        oxc: Some(serde_json::json!({
            "parseRegularExpression": true,
            "allowReturnOutsideFunction": true,
            "preserveParens": true,
            "allowV8Intrinsics": true,
            "enableIdentHashes": false,
        })),
        ..JsParserOptions::default()
    };

    let parsed: ParserOptions =
        options.to_options().expect("raw parser oxc parses");

    let oxc: PartialOxcParseOptions = parsed.oxc.expect("raw oxc present");

    assert_eq!(oxc.parse_regular_expression, Some(true));
    assert_eq!(oxc.allow_return_outside_function, Some(true));
    assert_eq!(oxc.preserve_parens, Some(true));
    assert_eq!(oxc.allow_v8_intrinsics, Some(true));
    assert_eq!(oxc.enable_ident_hashes, Some(false));
}

#[test]
fn test_generator_options_raw_oxc_accepts_camel_case_fields() {
    let options: JsGeneratorOptions = JsGeneratorOptions {
        oxc: Some(serde_json::json!({
            "singleQuote": true,
            "minify": true,
            "asciiOnly": true,
            "comments": {
                "normal": false,
                "jsdoc": true,
                "annotation": false,
                "legal": { "linked": "preserve-me" },
            },
            "indentChar": "tab",
            "indentWidth": 4,
            "initialIndent": 2,
        })),
        ..JsGeneratorOptions::default()
    };

    let parsed: GeneratorOptions =
        options.to_options().expect("raw codegen oxc parses");

    let oxc: PartialOxcCodegenOptions = parsed.oxc.expect("raw oxc present");

    assert_eq!(oxc.single_quote, Some(true));
    assert_eq!(oxc.minify, Some(true));
    assert_eq!(oxc.ascii_only, Some(true));

    let comments: PartialOxcCommentOptions =
        oxc.comments.expect("raw comment categories present");

    assert_eq!(comments.normal, Some(false));
    assert_eq!(comments.jsdoc, Some(true));
    assert_eq!(comments.annotation, Some(false));
    assert_eq!(
        comments.legal,
        Some(oxc::codegen::LegalComment::Linked(String::from("preserve-me")))
    );
    assert_eq!(oxc.indent_char, Some(oxc::codegen::IndentChar::Tab));
    assert_eq!(oxc.indent_width, Some(4));
    assert_eq!(oxc.initial_indent, Some(2));
}

#[test]
fn test_parser_options_raw_oxc_unknown_field_errors() {
    let options: JsParserOptions = JsParserOptions {
        oxc: Some(serde_json::json!({ "sourceMapPath": "ignored.map" })),
        ..JsParserOptions::default()
    };

    let error: napi::Error =
        options.to_options().expect_err("unknown raw field errors");

    assert_eq!(error.status, napi::Status::InvalidArg);
    assert!(error.reason.contains("invalid parser.oxc"), "{error}");
}

#[test]
fn test_generator_options_raw_oxc_unknown_field_errors() {
    let options: JsGeneratorOptions = JsGeneratorOptions {
        oxc: Some(serde_json::json!({ "sourceMapPath": "ignored.map" })),
        ..JsGeneratorOptions::default()
    };

    let error: napi::Error =
        options.to_options().expect_err("unknown raw field errors");

    assert_eq!(error.status, napi::Status::InvalidArg);
    assert!(error.reason.contains("invalid generator.oxc"), "{error}");
}

#[test]
fn test_generator_options_invalid_indent_style_errors() {
    let options: JsGeneratorOptions = JsGeneratorOptions {
        indent_style: Some(String::from("tabs")),
        ..JsGeneratorOptions::default()
    };

    let error: napi::Error =
        options.to_options().expect_err("invalid indent style errors");

    assert_eq!(error.status, napi::Status::InvalidArg);
    assert!(error.reason.contains("invalid indentStyle"), "{error}");
}

#[test]
fn test_generator_options_raw_oxc_invalid_indent_char_errors() {
    let options: JsGeneratorOptions = JsGeneratorOptions {
        oxc: Some(serde_json::json!({ "indentChar": "spaces" })),
        ..JsGeneratorOptions::default()
    };

    let error: napi::Error =
        options.to_options().expect_err("invalid indent char errors");

    assert_eq!(error.status, napi::Status::InvalidArg);
    assert!(error.reason.contains("invalid generator.oxc"), "{error}");
}

#[test]
fn test_parser_options_to_js_omits_absent_fields() {
    assert_eq!(
        parser_options_to_js(&ParserOptions::default()),
        serde_json::json!({})
    );
}

#[test]
fn test_parser_options_to_js_keeps_curated_and_raw_fields() {
    let options: ParserOptions = ParserOptions {
        preserve_parens: Some(true),
        allow_return_outside_function: Some(true),
        oxc: Some(PartialOxcParseOptions {
            enable_ident_hashes: Some(false),
            ..PartialOxcParseOptions::default()
        }),
    };

    assert_eq!(
        parser_options_to_js(&options),
        serde_json::json!({
            "preserveParens": true,
            "allowReturnOutsideFunction": true,
            "oxc": {
                "enableIdentHashes": false,
            },
        })
    );
}

#[test]
fn test_generator_options_to_js_omits_absent_raw_fields() {
    let options: GeneratorOptions = GeneratorOptions {
        single_quote: Some(true),
        oxc: Some(PartialOxcCodegenOptions {
            indent_width: Some(4),
            ..PartialOxcCodegenOptions::default()
        }),
        ..GeneratorOptions::default()
    };

    assert_eq!(
        generator_options_to_js(&options),
        serde_json::json!({
            "singleQuote": true,
            "oxc": {
                "indentWidth": 4,
            },
        })
    );
}

#[test]
fn test_generator_options_to_js_omits_absent_fields() {
    assert_eq!(
        generator_options_to_js(&GeneratorOptions::default()),
        serde_json::json!({})
    );
}

#[test]
fn test_generator_options_to_js_keeps_curated_and_raw_fields() {
    let options: GeneratorOptions = GeneratorOptions {
        single_quote: Some(true),
        indent_style: Some(IndentStyle::Tab),
        indent_width: Some(4),
        ..GeneratorOptions::default()
    };

    assert_eq!(
        generator_options_to_js(&options),
        serde_json::json!({
            "singleQuote": true,
            "indentStyle": "tab",
            "indentWidth": 4,
        })
    );
}

#[test]
fn test_language_label_roundtrip() {
    for (text, language) in [
        ("js", Language::JS),
        ("ts", Language::TS),
        ("dts", Language::DTS),
        ("jsx", Language::JSX),
        ("tsx", Language::TSX),
    ] {
        assert_eq!(language_label(Some(language)), text, "{text}");
    }
}

#[test]
fn test_source_type_label_roundtrip() {
    for (text, source_type) in [
        ("script", SourceType::Script),
        ("commonjs", SourceType::CommonJS),
        ("module", SourceType::Module),
        ("unambiguous", SourceType::Unambiguous),
    ] {
        assert_eq!(source_type_label(Some(source_type)), text, "{text}");
    }
}

#[test]
fn test_labels_empty_when_unspecified() {
    assert_eq!(language_label(None), "");
    assert_eq!(source_type_label(None), "");
}
