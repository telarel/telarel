use std::path::PathBuf;

use telarel_common::{
    GeneratorOptions, IndentStyle, Language, OutputOptions, ParserOptions,
    PartialOxcCodegenOptions, PartialOxcParseOptions, SourceMap,
    SourceMapOptions, SourceType,
};

use crate::_types::ast::JsAstInput;
use crate::_types::refs::{BagDescriptor, RefList};
use crate::ast::estree::ast_from_program;
use crate::ast::test_util::{fixture_ast, parse_fixture, sample_map};

use super::*;

// The descriptor loop runs only when the bag carries plugins, so the env
// is never dereferenced here. `Env::from_raw(null)` keeps the test free of
// a JS runtime.
fn env() -> Env {
    napi::Env::from_raw(std::ptr::null_mut())
}

/// Replaces the injected patch bridge in tests. The unmarshal tests never pass
/// descriptors, so an empty result is enough.
fn patch_bridge_stub(
    _env: &Env,
    _descriptors: Vec<ObjectRef<false>>,
    _refs: &Arc<RefList>,
) -> Result<Vec<SharedPluginable>> {
    Ok(Vec::new())
}

fn output(
    cwd: Option<String>,
    file: Option<String>,
    language: Option<String>,
    source_type: Option<String>,
) -> JsOptionsOutput {
    JsOptionsOutput {
        cwd,
        file,
        language,
        source_type,
        output: None,
        source_map: None,
        parser: None,
        generator: None,
        plugins: None,
    }
}

#[test]
fn test_unsupported_patch_plugins_empty_is_empty() {
    let env: Env = env();
    let refs: Arc<RefList> = Arc::new(RefList::new());

    let plugins: Vec<SharedPluginable> =
        unsupported_patch_plugins(&env, Vec::new(), &refs)
            .expect("an empty descriptor list needs no bridge");

    assert!(plugins.is_empty());
    assert!(refs.is_empty());
}

#[test]
fn test_hook_error_decodes_a_plugin_envelope() {
    let reason: String =
        String::from("__TELAREL_PLUGIN_ERROR__:{\"message\":\"boom\"}");

    let error: PluginError =
        hook_error(napi::Error::new(napi::Status::GenericFailure, reason));

    assert_eq!(error.message(), "boom");
    assert!(error.source().is_none());
}

#[test]
fn test_hook_error_keeps_ordinary_napi_rendering() {
    let error: PluginError =
        hook_error(napi::Error::new(napi::Status::GenericFailure, "kaboom"));

    assert_eq!(error.message(), "GenericFailure, kaboom");
}

#[test]
fn test_unmarshal_options_null_is_none() {
    let env: Env = env();
    let refs: Arc<RefList> = Arc::new(RefList::new());

    let outcome =
        unmarshal_options(None, &env, &refs, patch_bridge_stub).unwrap();

    assert!(outcome.is_none());
}

#[test]
fn test_unmarshal_options_bag_keeps_omitted_fields_none() {
    let env: Env = env();
    let refs: Arc<RefList> = Arc::new(RefList::new());

    let outcome = unmarshal_options(
        Some(output(None, None, None, None)),
        &env,
        &refs,
        patch_bridge_stub,
    )
    .unwrap()
    .expect("a returned bag is Some");

    assert_eq!(outcome.options.cwd, None);
    assert_eq!(outcome.options.file, None);
    assert_eq!(outcome.options.language, None);
    assert_eq!(outcome.options.source_type, None);
    assert!(outcome.options.output.is_none());
    assert!(outcome.options.source_map.is_none());
    assert!(outcome.options.parser.is_none());
    assert!(outcome.options.generator.is_none());
    assert!(outcome.plugins.is_empty());
}

#[test]
fn test_unmarshal_options_bag_preserves_echoed_groups() {
    let env: Env = env();
    let refs: Arc<RefList> = Arc::new(RefList::new());

    let mut bag: JsOptionsOutput = output(None, None, None, None);

    bag.parser = Some(serde_json::json!({
        "preserveParens": true,
        "allowReturnOutsideFunction": true,
        "oxc": { "enableIdentHashes": false },
    }));

    bag.generator = Some(serde_json::json!({
        "singleQuote": true,
        "asciiOnly": true,
        "indentStyle": "tab",
        "indentWidth": 4,
        "comments": false,
        "minify": true,
        "oxc": { "initialIndent": 1 },
    }));

    let outcome: OptionsArgs =
        unmarshal_options(Some(bag), &env, &refs, patch_bridge_stub)
            .unwrap()
            .expect("a returned bag is Some");

    let parser: ParserOptions =
        outcome.options.parser.expect("the parser group is carried");

    assert_eq!(parser.preserve_parens, Some(true));
    assert_eq!(parser.allow_return_outside_function, Some(true));
    assert_eq!(
        parser.oxc.expect("raw oxc is carried").enable_ident_hashes,
        Some(false)
    );

    let generator: GeneratorOptions =
        outcome.options.generator.expect("the generator group is carried");

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
fn test_options_call_payload_carries_stage_groups() {
    let options: ResolvableOptions = ResolvableOptions {
        parser: Some(ParserOptions {
            preserve_parens: Some(true),
            allow_return_outside_function: Some(true),
            oxc: Some(PartialOxcParseOptions {
                enable_ident_hashes: Some(false),
                ..PartialOxcParseOptions::default()
            }),
        }),
        generator: Some(GeneratorOptions {
            single_quote: Some(true),
            indent_style: Some(IndentStyle::Tab),
            oxc: Some(PartialOxcCodegenOptions {
                indent_width: Some(4),
                ..PartialOxcCodegenOptions::default()
            }),
            ..GeneratorOptions::default()
        }),
        ..ResolvableOptions::default()
    };

    let call: OptionsCall = options_call(&options, Vec::new());

    let parser: serde_json::Value =
        call.parser.expect("the payload carries the parser group");
    assert_eq!(parser["preserveParens"], serde_json::json!(true));
    assert_eq!(parser["allowReturnOutsideFunction"], serde_json::json!(true));
    assert_eq!(
        parser["oxc"],
        serde_json::json!({ "enableIdentHashes": false })
    );

    let generator: serde_json::Value =
        call.generator.expect("the payload carries the generator group");
    assert_eq!(generator["singleQuote"], serde_json::json!(true));
    assert_eq!(generator["indentStyle"], serde_json::json!("tab"));
    assert_eq!(generator["oxc"], serde_json::json!({ "indentWidth": 4 }));
}

#[test]
fn test_options_call_payload_omits_absent_stage_groups() {
    let call: OptionsCall =
        options_call(&ResolvableOptions::default(), Vec::new());

    assert!(call.parser.is_none());
    assert!(call.generator.is_none());
}

#[test]
fn test_unmarshal_options_bag_invalid_parser_group_errors() {
    let env: Env = env();
    let refs: Arc<RefList> = Arc::new(RefList::new());

    let mut bag: JsOptionsOutput = output(None, None, None, None);

    bag.parser = Some(serde_json::json!({ "sourceMapPath": "a.map" }));

    let error: PluginError =
        unmarshal_options(Some(bag), &env, &refs, patch_bridge_stub)
            .expect_err("an unknown parser field is the hook error");

    assert!(error.to_string().contains("invalid parser options"), "{error}");
}

#[test]
fn test_unmarshal_options_bag_invalid_generator_group_errors() {
    let env: Env = env();
    let refs: Arc<RefList> = Arc::new(RefList::new());

    let mut bag: JsOptionsOutput = output(None, None, None, None);

    bag.generator = Some(serde_json::json!({
        "indentStyle": "tabs",
    }));

    let error: PluginError =
        unmarshal_options(Some(bag), &env, &refs, patch_bridge_stub)
            .expect_err("an invalid generator field is the hook error");

    assert!(error.to_string().contains("invalid generator options"), "{error}");
}

#[test]
fn test_unmarshal_options_bag_invalid_raw_oxc_errors() {
    let env: Env = env();
    let refs: Arc<RefList> = Arc::new(RefList::new());

    let mut bag: JsOptionsOutput = output(None, None, None, None);

    bag.parser =
        Some(serde_json::json!({ "oxc": { "sourceMapPath": "a.map" } }));

    let error: PluginError =
        unmarshal_options(Some(bag), &env, &refs, patch_bridge_stub)
            .expect_err("an unknown raw parser field is the hook error");

    assert!(error.to_string().contains("invalid parser.oxc"), "{error}");

    let mut bag: JsOptionsOutput = output(None, None, None, None);

    bag.generator =
        Some(serde_json::json!({ "oxc": { "indentChar": "spaces" } }));

    let error: PluginError =
        unmarshal_options(Some(bag), &env, &refs, patch_bridge_stub)
            .expect_err("an invalid raw indentChar is the hook error");

    assert!(error.to_string().contains("invalid generator.oxc"), "{error}");
}

#[test]
fn test_unmarshal_options_bag_parses_output() {
    let env: Env = env();
    let refs: Arc<RefList> = Arc::new(RefList::new());

    let mut bag: JsOptionsOutput = output(None, None, None, None);

    bag.output = Some(crate::_types::options::JsOutputOptions {
        file: Some(String::from("dist/index.js")),
    });

    let outcome: OptionsArgs =
        unmarshal_options(Some(bag), &env, &refs, patch_bridge_stub)
            .unwrap()
            .expect("a returned bag is Some");

    let output: OutputOptions =
        outcome.options.output.expect("the bag's output is carried");

    assert_eq!(output.file.as_deref(), Some("dist/index.js"));
}

#[test]
fn test_unmarshal_options_bag_parses_source_map() {
    let env: Env = env();
    let refs: Arc<RefList> = Arc::new(RefList::new());

    let mut bag: JsOptionsOutput = output(None, None, None, None);

    bag.source_map = Some(serde_json::json!({
        "input": "inline",
        "output": { "mode": "both" },
    }));

    let outcome = unmarshal_options(Some(bag), &env, &refs, patch_bridge_stub)
        .unwrap()
        .expect("a returned bag is Some");

    let source_map =
        outcome.options.source_map.expect("the bag's sourceMap is carried");

    assert!(matches!(
        source_map.input,
        Some(telarel_common::InputSourceMap::Mode(
            telarel_common::InputSourceMapMode::Inline
        ))
    ));

    assert!(matches!(
        source_map.output,
        Some(telarel_common::OutputSourceMap::Options(
            telarel_common::OutputSourceMapOptions {
                mode: telarel_common::OutputSourceMapMode::Both,
                url: None,
            }
        ))
    ));
}

#[test]
fn test_unmarshal_options_bag_parses_source_map_paths() {
    let env: Env = env();
    let refs: Arc<RefList> = Arc::new(RefList::new());

    let mut bag: JsOptionsOutput = output(None, None, None, None);

    bag.source_map = Some(serde_json::json!({
        "input": {
            "mode": "external",
            "path": "maps/in.map",
        },
        "output": {
            "mode": "external",
            "url": "dist/out.js.map",
        },
    }));

    let outcome: OptionsArgs =
        unmarshal_options(Some(bag), &env, &refs, patch_bridge_stub)
            .unwrap()
            .expect("a returned bag is Some");

    let source_map: SourceMapOptions =
        outcome.options.source_map.expect("the bag's sourceMap is carried");

    assert!(matches!(
        source_map.input,
        Some(telarel_common::InputSourceMap::Options(
            telarel_common::InputSourceMapOptions {
                mode: telarel_common::InputSourceMapMode::External,
                path: Some(path),
            }
        )) if path == std::path::Path::new("maps/in.map")
    ));

    assert!(matches!(
        source_map.output,
        Some(telarel_common::OutputSourceMap::Options(
            telarel_common::OutputSourceMapOptions {
                mode: telarel_common::OutputSourceMapMode::External,
                url: Some(url),
            }
        )) if url == "dist/out.js.map"
    ));
}

#[test]
fn test_unmarshal_options_bag_invalid_source_map_errors() {
    let env: Env = env();
    let refs: Arc<RefList> = Arc::new(RefList::new());

    let mut bag: JsOptionsOutput = output(None, None, None, None);

    bag.source_map = Some(serde_json::json!({ "output": "sideways" }));

    let error: PluginError =
        unmarshal_options(Some(bag), &env, &refs, patch_bridge_stub)
            .expect_err("an invalid sourceMap is the hook error");

    assert!(error.to_string().contains("invalid sourceMap.output"), "{error}");
}

#[test]
fn test_unmarshal_options_bag_fields_and_labels() {
    let env: Env = env();
    let refs: Arc<RefList> = Arc::new(RefList::new());

    let outcome = unmarshal_options(
        Some(output(
            Some("/tmp".to_owned()),
            Some("app.ts".to_owned()),
            Some("ts".to_owned()),
            Some("module".to_owned()),
        )),
        &env,
        &refs,
        patch_bridge_stub,
    )
    .unwrap()
    .expect("a returned bag is Some");

    assert_eq!(outcome.options.cwd, Some(PathBuf::from("/tmp")));
    assert_eq!(outcome.options.file.as_deref(), Some("app.ts"));
    assert_eq!(outcome.options.language, Some(Language::TS));
    assert_eq!(outcome.options.source_type, Some(SourceType::Module));
    assert!(outcome.plugins.is_empty());
}

#[test]
fn test_unmarshal_options_omitted_plugins_keeps_the_bag_table() {
    let env: Env = env();
    let refs: Arc<RefList> = Arc::new(RefList::new());

    refs.seed_bag(vec![
        BagDescriptor::stub(Some(1)),
        BagDescriptor::stub(Some(2)),
    ]);

    assert_eq!(refs.bag_snapshot().len(), 2);

    unmarshal_options(
        Some(output(None, None, None, None)),
        &env,
        &refs,
        patch_bridge_stub,
    )
    .unwrap()
    .expect("a returned bag is Some");

    assert_eq!(refs.bag_ids(), vec![Some(1), Some(2)]);
}

/// The contract: the `options` payload is the settled descriptor view, not
/// the accumulating release list. The entry seed stays in place, and a
/// later patch appends only its new descriptor. An echoed descriptor whose
/// id is already in the view adds nothing.
///
/// Limitation: the descriptor-loop half of `unmarshal_options` touches
/// napi FFI (`materialize` -> `get_value`) and cannot run without a JS
/// runtime. The test therefore models the patch append with the same
/// `append_bag` calls that the loop makes.
#[test]
fn test_bag_table_tracks_settled_view_not_release_list() {
    let refs: RefList = RefList::new();

    let first: Vec<BagDescriptor> =
        vec![BagDescriptor::stub(Some(1)), BagDescriptor::stub(Some(2))];

    for descriptor in &first {
        refs.push_ref(descriptor.reference.clone());
    }

    refs.seed_bag(first);

    assert_eq!(refs.bag_snapshot().len(), 2);
    assert_eq!(refs.len(), 2);

    // The patch echoes descriptor 2 and adds descriptor 3.
    let patch: Vec<BagDescriptor> =
        vec![BagDescriptor::stub(Some(2)), BagDescriptor::stub(Some(3))];

    for descriptor in &patch {
        refs.push_ref(descriptor.reference.clone());
    }

    refs.append_bag(patch);

    assert_eq!(refs.bag_ids(), vec![Some(1), Some(2), Some(3)]);
    assert_eq!(refs.len(), 4);
}

#[test]
fn test_unmarshal_options_invalid_language_errors() {
    let env: Env = env();
    let refs: Arc<RefList> = Arc::new(RefList::new());

    let outcome = unmarshal_options(
        Some(output(None, None, Some("json".to_owned()), None)),
        &env,
        &refs,
        patch_bridge_stub,
    );

    let error: PluginError =
        outcome.expect_err("an invalid language is the hook error");

    assert!(error.to_string().contains("invalid language"), "{error}");
}

#[test]
fn test_unmarshal_options_invalid_source_type_errors() {
    let env: Env = env();
    let refs: Arc<RefList> = Arc::new(RefList::new());

    let outcome = unmarshal_options(
        Some(output(None, None, None, Some("json".to_owned()))),
        &env,
        &refs,
        patch_bridge_stub,
    );

    let error: PluginError =
        outcome.expect_err("an invalid source type is the hook error");

    assert!(error.to_string().contains("invalid source type"), "{error}");
}

#[test]
fn test_transform_read_back_returned_language_wins() {
    let language: Language =
        transform_read_back_language(Some("jsx"), Language::TS)
            .expect("a returned language parses");

    assert_eq!(language, Language::JSX);
}

#[test]
fn test_transform_read_back_absent_language_falls_back_to_input() {
    let language: Language = transform_read_back_language(None, Language::TSX)
        .expect("absent language is unset");

    assert_eq!(language, Language::TSX);
}

#[test]
fn test_transform_read_back_empty_language_is_unset() {
    let language: Language =
        transform_read_back_language(Some(""), Language::JS)
            .expect("an empty label is unset");

    assert_eq!(language, Language::JS);
}

#[test]
fn test_transform_read_back_invalid_language_errors() {
    let error: PluginError =
        transform_read_back_language(Some("json"), Language::JS)
            .expect_err("an invalid label is the hook error");

    assert!(error.to_string().contains("invalid language"), "{error}");
}

#[test]
fn test_transform_read_back_defers_module_kind_to_program_source_type() {
    let language: Language =
        transform_read_back_language(Some("tsx"), Language::JS)
            .expect("the returned language parses");

    let source_type: oxc::span::SourceType = source_type_for_generate(language);

    assert!(source_type.is_unambiguous(), "{source_type:?}");

    let program: serde_json::Value = serde_json::json!({
        "type": "Program",
        "sourceType": "module",
        "body": [],
        "start": 0,
        "end": 0,
    });

    let ast: Ast = ast_from_program(&program, String::new(), None, source_type)
        .expect("the program decodes");

    assert_eq!(ast.language(), Language::TSX);
    assert_eq!(ast.source_type(), SourceType::Module);
    assert!(ast.program().source_type.is_module());
}

fn transform_input(ast: Ast) -> TransformInput {
    let (ast_json, code, map, _language): (
        String,
        String,
        Option<JsSourceMap>,
        String,
    ) = ast_hook_parts(&ast);

    TransformInput { ast, ast_json, code, map }
}

fn unchanged_output(input: &TransformInput) -> JsTransformOutput {
    JsTransformOutput {
        ast_json: input.ast_json.clone(),
        code: input.code.clone(),
        map: input.map.clone(),
        language: Some(input.ast.language().as_str().to_string()),
    }
}

fn replaced_output(
    ast: &Ast,
    language: Option<&str>,
) -> JsTransformOutput {
    let (ast_json, code, map, _language): (
        String,
        String,
        Option<JsSourceMap>,
        String,
    ) = ast_hook_parts(ast);

    JsTransformOutput {
        ast_json,
        code,
        map,
        language: language.map(ToString::to_string),
    }
}

#[test]
fn test_transform_hook_set_requires_both_hooks() {
    assert!(TransformHookSet { transform: true, batch: true }.is_batchable());
    assert!(!TransformHookSet { transform: true, batch: false }.is_batchable());
    assert!(!TransformHookSet { transform: false, batch: true }.is_batchable());
    assert!(
        !TransformHookSet { transform: false, batch: false }.is_batchable()
    );
}

#[test]
fn test_finish_transform_output_identical_returns_none() {
    let input: TransformInput = transform_input(fixture_ast("const a = 1;"));

    let replaced: JsTransformOutput = unchanged_output(&input);

    let output: Option<TransformOutput> =
        finish_transform_output(replaced, input)
            .expect("an identical pass-through read-back succeeds");

    assert!(output.is_none());
}

#[test]
fn test_finish_transform_output_changed_json_rebuilds_some() {
    let input: TransformInput = transform_input(fixture_ast("const a = 1;"));

    let replacement: Ast = fixture_ast("\"changed\";");

    let replaced: JsTransformOutput = replaced_output(&replacement, None);

    let output: TransformOutput = finish_transform_output(replaced, input)
        .expect("a changed read-back succeeds")
        .expect("a changed read-back returns Some");

    let generated: String =
        oxc::codegen::Codegen::new().build(output.ast.program()).code;

    assert!(generated.contains("changed"), "{generated}");
}

#[test]
fn test_finish_transform_output_absent_language_falls_back_to_input() {
    let input: TransformInput =
        transform_input(parse_fixture("const a: number = 1;", Language::TS));

    let replacement: Ast = parse_fixture("const a: number = 2;", Language::TS);

    let replaced: JsTransformOutput = replaced_output(&replacement, None);

    let output: TransformOutput = finish_transform_output(replaced, input)
        .expect("the read-back succeeds")
        .expect("the change returns Some");

    assert_eq!(output.ast.language(), Language::TS);
}

#[test]
fn test_finish_transform_output_language_only_change_rebuilds() {
    let input: TransformInput =
        transform_input(parse_fixture("const a = 1;", Language::JS));

    let mut replaced: JsTransformOutput = unchanged_output(&input);

    replaced.language = Some(String::from("jsx"));

    let output: TransformOutput = finish_transform_output(replaced, input)
        .expect("the read-back succeeds")
        .expect("a grammar-only change is not a no-op");

    assert_eq!(output.ast.language(), Language::JSX);
}

#[test]
fn test_finish_transform_output_identical_comments_reanchors() {
    let input: TransformInput =
        transform_input(parse_fixture("const a = 1; // tail\n", Language::JS));

    let replaced: JsTransformOutput = unchanged_output(&input);

    let output: TransformOutput = finish_transform_output(replaced, input)
        .expect("the re-anchor succeeds")
        .expect("comments that need re-anchoring return Some");

    assert_eq!(output.ast.program().comments.len(), 1);
    assert!(output.ast.program().comments[0].is_leading());
}

#[test]
fn test_finish_transform_batch_output_no_member_change_returns_none() {
    let input: TransformInput = transform_input(fixture_ast("const a = 1;"));

    let output: Option<TransformOutput> =
        finish_transform_batch_output(None, input)
            .expect("a batch with no accepted change succeeds");

    assert!(output.is_none());
}

#[test]
fn test_finish_transform_batch_output_pass_through_returns_none() {
    let input: TransformInput = transform_input(fixture_ast("const a = 1;"));

    let replaced: JsTransformOutput = unchanged_output(&input);

    let output: Option<TransformOutput> =
        finish_transform_batch_output(Some(replaced), input)
            .expect("a pass-through batch read-back succeeds");

    assert!(output.is_none());
}

#[test]
fn test_finish_transform_batch_output_changed_member_rebuilds_code_map_and_comments()
 {
    let input: TransformInput = transform_input(fixture_ast("const a = 1;"));

    let replacement: Ast = fixture_ast("const changed = 1; // tail\n");

    let map: SourceMap = sample_map();

    let mut replaced: JsTransformOutput = replaced_output(&replacement, None);

    replaced.map = Some(source_map_to_js(&map));

    let output: TransformOutput =
        finish_transform_batch_output(Some(replaced), input)
            .expect("a changed batch read-back succeeds")
            .expect("a changed batch member returns Some");

    let generated: String =
        oxc::codegen::Codegen::new().build(output.ast.program()).code;

    assert!(generated.contains("changed"), "{generated}");
    assert_eq!(output.ast.program().comments.len(), 1);

    let output_map: &SourceMap =
        output.ast.map().expect("the returned map is rebuilt");

    assert_eq!(output_map.to_json_string(), map.to_json_string());
}

fn parse_output(
    program: serde_json::Value,
    language: Option<String>,
) -> JsParseOutput {
    JsParseOutput {
        ast: JsAstInput { program, code: String::new(), map: None, language },
    }
}

fn empty_module_program() -> serde_json::Value {
    serde_json::json!({
        "type": "Program",
        "sourceType": "module",
        "body": [],
        "start": 0,
        "end": 0,
    })
}

#[test]
fn test_parse_output_ast_without_language_rebuilds_with_parser_options() {
    let output: JsParseOutput = parse_output(empty_module_program(), None);

    let source_type: oxc::span::SourceType = source_type_for_options(
        "index.ts",
        Some(Language::TS),
        SourceType::Unambiguous,
    );

    let ast: Ast = js_to_ast(output.ast, source_type)
        .expect("an absent returned language is unset");

    assert_eq!(ast.language(), Language::TS);
    assert_eq!(ast.source_type(), SourceType::Module);
}

#[test]
fn test_parse_output_ast_language_is_ignored_for_grammar() {
    let output: JsParseOutput =
        parse_output(empty_module_program(), Some(String::from("jsx")));

    let source_type: oxc::span::SourceType = source_type_for_options(
        "index.ts",
        Some(Language::TS),
        SourceType::Unambiguous,
    );

    let ast: Ast = js_to_ast(output.ast, source_type)
        .expect("a returned language does not block read-back");

    assert_eq!(ast.language(), Language::TS);
    assert!(ast.program().source_type.is_typescript());
}
