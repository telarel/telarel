use std::path::PathBuf;

use napi::bindgen_prelude::ObjectRef;
use napi::{Error, Result, Status};

use telarel_common::{
    CompileFromAstOptions, CompileOptions, GenerateOptions, InputSourceMap,
    InputSourceMapMode, Language, OutputSourceMap, OutputSourceMapMode,
    OutputSourceMapOptions, ParseOptions, ResolvableOptions, RewriteOptions,
    SourceMap, SourceMapOptions, SourceType,
};

/// JS-facing options for `parse`.
#[napi_derive::napi(object)]
pub struct JsParseOptions {
    /// Current working directory;
    /// omitted on the JS side resolves to the process working directory.
    pub cwd: Option<String>,
    /// The file being parsed;
    /// omitted on the JS side resolves to a default filename.
    pub file: Option<String>,
    /// The grammar of the source;
    /// omitted on the JS side infers the grammar from the file extension.
    pub language: Option<String>,
    /// The module system of the source;
    /// omitted on the JS side keeps the module kind resolved from the grammar.
    #[napi(js_name = "sourceType")]
    pub source_type: Option<String>,
    /// The source-map inputs and outputs requested for the step.
    #[napi(js_name = "sourceMap")]
    pub source_map: Option<serde_json::Value>,
    /// Raw JS plugin objects, rooted as references;
    /// bridged by `JsPlugin::from_object`.
    pub plugins: Vec<ObjectRef<false>>,
}

/// JS-facing options for `compile`.
#[napi_derive::napi(object)]
pub struct JsCompileOptions {
    /// Current working directory;
    /// omitted on the JS side resolves to the process working directory.
    pub cwd: Option<String>,
    /// The file being compiled;
    /// omitted on the JS side resolves to a default filename.
    pub file: Option<String>,
    /// The grammar of the source;
    /// omitted on the JS side infers the grammar from the file extension.
    pub language: Option<String>,
    /// The module system of the source;
    /// omitted on the JS side keeps the module kind resolved from the grammar.
    #[napi(js_name = "sourceType")]
    pub source_type: Option<String>,
    /// The source-map inputs and outputs requested for the step.
    #[napi(js_name = "sourceMap")]
    pub source_map: Option<serde_json::Value>,
    /// Raw JS plugin objects, rooted as references;
    /// bridged by `JsPlugin::from_object`.
    pub plugins: Vec<ObjectRef<false>>,
}

/// JS-facing options for `rewrite`.
#[napi_derive::napi(object)]
pub struct JsRewriteOptions {
    /// Current working directory;
    /// omitted on the JS side resolves to the process working directory.
    pub cwd: Option<String>,
    /// The file being rewritten;
    /// omitted on the JS side resolves to a default filename.
    pub file: Option<String>,
    /// The grammar of the source;
    /// omitted on the JS side infers the grammar from the file extension.
    pub language: Option<String>,
    /// The module system of the source;
    /// omitted on the JS side keeps the module kind resolved from the grammar.
    #[napi(js_name = "sourceType")]
    pub source_type: Option<String>,
    /// The source-map inputs and outputs requested for the step.
    #[napi(js_name = "sourceMap")]
    pub source_map: Option<serde_json::Value>,
    /// Raw JS plugin objects, rooted as references;
    /// bridged by `JsPlugin::from_object`.
    pub plugins: Vec<ObjectRef<false>>,
}

/// JS-facing options for `compileFromAst`.
#[napi_derive::napi(object)]
pub struct JsCompileFromAstOptions {
    /// Current working directory;
    /// omitted on the JS side resolves to the process working directory.
    pub cwd: Option<String>,
    /// The file being compiled;
    /// omitted on the JS side resolves to a default filename.
    pub file: Option<String>,
    /// The grammar of the source;
    /// omitted on the JS side infers the grammar from the file extension.
    pub language: Option<String>,
    /// The module system of the source;
    /// omitted on the JS side keeps the module kind resolved from the grammar.
    #[napi(js_name = "sourceType")]
    pub source_type: Option<String>,
    /// The source-map inputs and outputs requested for the step.
    #[napi(js_name = "sourceMap")]
    pub source_map: Option<serde_json::Value>,
    /// Raw JS plugin objects, rooted as references;
    /// bridged by `JsPlugin::from_object`.
    pub plugins: Vec<ObjectRef<false>>,
}

/// JS-facing options for `generate`: no grammar and no module kind, because
/// the generated program's own `sourceType` settles the module kind.
#[napi_derive::napi(object)]
pub struct JsGenerateOptions {
    /// Current working directory;
    /// omitted on the JS side resolves to the process working directory.
    pub cwd: Option<String>,
    /// The file being generated;
    /// omitted on the JS side resolves to a default filename.
    pub file: Option<String>,
    /// The source-map inputs and outputs requested for the step.
    #[napi(js_name = "sourceMap")]
    pub source_map: Option<serde_json::Value>,
    /// Raw JS plugin objects, rooted as references;
    /// bridged by `JsPlugin::from_object`.
    pub plugins: Vec<ObjectRef<false>>,
}

/// JS-facing options for `builtinParse` (the builtin `createParser`): no
/// plugins, since the builtin parser never runs the plugin chain.
#[napi_derive::napi(object)]
pub struct JsParserOptions {
    /// Current working directory;
    /// omitted on the JS side resolves to the process working directory.
    pub cwd: Option<String>,
    /// The file being parsed;
    /// omitted on the JS side resolves to a default filename.
    pub file: Option<String>,
    /// The grammar of the source;
    /// omitted on the JS side infers the grammar from the file extension.
    pub language: Option<String>,
    /// The module system of the source;
    /// omitted on the JS side keeps the module kind resolved from the grammar.
    #[napi(js_name = "sourceType")]
    pub source_type: Option<String>,
    /// The source-map inputs and outputs requested for the step.
    #[napi(js_name = "sourceMap")]
    pub source_map: Option<serde_json::Value>,
}

/// JS-facing options for `builtinGenerate` (the builtin `createGenerator`):
/// no plugins, no grammar and no module kind.
#[napi_derive::napi(object)]
pub struct JsGeneratorOptions {
    /// Current working directory;
    /// omitted on the JS side resolves to the process working directory.
    pub cwd: Option<String>,
    /// The file being generated;
    /// omitted on the JS side resolves to a default filename.
    pub file: Option<String>,
    /// The source-map inputs and outputs requested for the step.
    #[napi(js_name = "sourceMap")]
    pub source_map: Option<serde_json::Value>,
}

/// Wrap a validation message in an `InvalidArg` error.
fn invalid_arg(message: String) -> Error {
    Error::new(Status::InvalidArg, message)
}

/// Parse a JS-facing source type string into the Rust enum.
fn parse_source_type(text: &str) -> std::result::Result<SourceType, String> {
    match text {
        | "script" => Ok(SourceType::Script),
        | "commonjs" => Ok(SourceType::CommonJS),
        | "module" => Ok(SourceType::Module),
        | "unambiguous" => Ok(SourceType::Unambiguous),
        | _ => Err(format!(
            "invalid source type {text:?}: expected \"script\", \"commonjs\", \"module\" or \"unambiguous\""
        )),
    }
}

/// Parse a JS source-map object into an owned [`SourceMap`].
fn parse_source_map_object(value: &serde_json::Value) -> Result<SourceMap> {
    let json: String = serde_json::to_string(value).map_err(|error| {
        invalid_arg(format!("invalid sourceMap.input object: {error}"))
    })?;

    oxc_sourcemap::OwnedSourceMap::from_json_string(&json)
        .map(oxc_sourcemap::OwnedSourceMap::into_inner)
        .map_err(|error| {
            invalid_arg(format!("invalid sourceMap.input object: {error}"))
        })
}

/// Parse the `sourceMap.input` union: a source-map object, a path string, or
/// one of `"inline"` / `"external"` / `"auto"`.
fn parse_input_source_map(value: &serde_json::Value) -> Result<InputSourceMap> {
    match value {
        | serde_json::Value::Object(_) => Ok(InputSourceMap::Mode(
            InputSourceMapMode::Value(parse_source_map_object(value)?),
        )),
        | serde_json::Value::String(text) => {
            let mode: InputSourceMapMode = match text.as_str() {
                | "inline" => InputSourceMapMode::Inline,
                | "external" => InputSourceMapMode::External,
                | "auto" => InputSourceMapMode::Auto,
                | path => InputSourceMapMode::Path(PathBuf::from(path)),
            };

            Ok(InputSourceMap::Mode(mode))
        },
        | _ => Err(invalid_arg(
            "invalid sourceMap.input: expected a source map object, a path \
             string, or \"inline\" / \"external\" / \"auto\""
                .to_owned(),
        )),
    }
}

/// Parse one output source-map mode label.
fn parse_output_source_map_mode(text: &str) -> Result<OutputSourceMapMode> {
    match text {
        | "inline" => Ok(OutputSourceMapMode::Inline),
        | "external" => Ok(OutputSourceMapMode::External),
        | "both" => Ok(OutputSourceMapMode::Both),
        | _ => Err(invalid_arg(format!(
            "invalid sourceMap.output mode {text:?}: expected \"inline\", \
             \"external\" or \"both\""
        ))),
    }
}

/// Parse the `sourceMap.output` union: a mode string or `{ mode }`.
fn parse_output_source_map(
    value: &serde_json::Value
) -> Result<OutputSourceMap> {
    match value {
        | serde_json::Value::String(text) => {
            Ok(OutputSourceMap::Mode(parse_output_source_map_mode(text)?))
        },
        | serde_json::Value::Object(object) => {
            let mode: &serde_json::Value =
                object.get("mode").ok_or_else(|| {
                    invalid_arg(
                        "invalid sourceMap.output: an options object requires \
                         a `mode`"
                            .to_owned(),
                    )
                })?;

            let text: &str = mode.as_str().ok_or_else(|| {
                invalid_arg(
                    "invalid sourceMap.output.mode: expected a string"
                        .to_owned(),
                )
            })?;

            Ok(OutputSourceMap::Options(OutputSourceMapOptions {
                mode: parse_output_source_map_mode(text)?,
            }))
        },
        | _ => Err(invalid_arg(
            "invalid sourceMap.output: expected \"inline\" / \"external\" / \
             \"both\" or { mode }"
                .to_owned(),
        )),
    }
}

/// Parse the JS `sourceMap` union into the Rust option family.
///
/// `null` and absent keys are "off" (`None`) — never `"auto"`. A malformed
/// shape is a hard `InvalidArg` error, never a silent default.
pub fn parse_source_map_options(
    value: &serde_json::Value
) -> Result<SourceMapOptions> {
    let object: &serde_json::Map<String, serde_json::Value> =
        value.as_object().ok_or_else(|| {
            invalid_arg("invalid sourceMap: expected an object".to_owned())
        })?;

    let input: Option<InputSourceMap> = match object.get("input") {
        | Some(serde_json::Value::Null) | None => None,
        | Some(value) => Some(parse_input_source_map(value)?),
    };

    let output: Option<OutputSourceMap> = match object.get("output") {
        | Some(serde_json::Value::Null) | None => None,
        | Some(value) => Some(parse_output_source_map(value)?),
    };

    Ok(SourceMapOptions { input, output })
}

/// The parsed, shared scalar fields of a pipeline option struct.
///
/// `parse`, `compile`, `rewrite` and `compileFromAst` carry the same five
/// scalars on the wire; only the plugins array (bridged separately) is
/// specific to each entry point.
struct SharedOptions {
    cwd: Option<String>,
    file: Option<String>,
    language: Option<Language>,
    source_type: Option<SourceType>,
    source_map: Option<SourceMapOptions>,
}

impl SharedOptions {
    /// Parse the shared scalars, validating the grammar and module-kind
    /// labels and the `sourceMap` union.
    fn parse(
        cwd: Option<String>,
        file: Option<String>,
        language: Option<&str>,
        source_type: Option<&str>,
        source_map: Option<&serde_json::Value>,
    ) -> Result<Self> {
        let language: Option<Language> = match language {
            | Some(text) => {
                Some(text.parse::<Language>().map_err(invalid_arg)?)
            },
            | None => None,
        };

        let source_type: Option<SourceType> = match source_type {
            | Some(text) => Some(parse_source_type(text).map_err(invalid_arg)?),
            | None => None,
        };

        let source_map: Option<SourceMapOptions> = match source_map {
            | Some(value) => Some(parse_source_map_options(value)?),
            | None => None,
        };

        Ok(Self { cwd, file, language, source_type, source_map })
    }
}

impl JsParseOptions {
    /// Convert the JS-facing options into the Rust [`ParseOptions`].
    pub fn to_parse_options(&self) -> Result<ParseOptions> {
        let shared: SharedOptions = SharedOptions::parse(
            self.cwd.clone(),
            self.file.clone(),
            self.language.as_deref(),
            self.source_type.as_deref(),
            self.source_map.as_ref(),
        )?;

        Ok(ParseOptions {
            cwd: shared.cwd,
            file: shared.file,
            language: shared.language,
            source_type: shared.source_type,
            source_map: shared.source_map,
        })
    }
}

impl JsCompileOptions {
    /// Convert the JS-facing options into the Rust [`CompileOptions`].
    pub fn to_compile_options(&self) -> Result<CompileOptions> {
        let shared: SharedOptions = SharedOptions::parse(
            self.cwd.clone(),
            self.file.clone(),
            self.language.as_deref(),
            self.source_type.as_deref(),
            self.source_map.as_ref(),
        )?;

        Ok(CompileOptions {
            cwd: shared.cwd,
            file: shared.file,
            language: shared.language,
            source_type: shared.source_type,
            source_map: shared.source_map,
        })
    }
}

impl JsRewriteOptions {
    /// Convert the JS-facing options into the Rust [`RewriteOptions`].
    pub fn to_rewrite_options(&self) -> Result<RewriteOptions> {
        let shared: SharedOptions = SharedOptions::parse(
            self.cwd.clone(),
            self.file.clone(),
            self.language.as_deref(),
            self.source_type.as_deref(),
            self.source_map.as_ref(),
        )?;

        Ok(RewriteOptions {
            cwd: shared.cwd,
            file: shared.file,
            language: shared.language,
            source_type: shared.source_type,
            source_map: shared.source_map,
        })
    }
}

impl JsCompileFromAstOptions {
    /// Convert the JS-facing options into the Rust [`CompileFromAstOptions`].
    pub fn to_compile_from_ast_options(&self) -> Result<CompileFromAstOptions> {
        let shared: SharedOptions = SharedOptions::parse(
            self.cwd.clone(),
            self.file.clone(),
            self.language.as_deref(),
            self.source_type.as_deref(),
            self.source_map.as_ref(),
        )?;

        Ok(CompileFromAstOptions {
            cwd: shared.cwd,
            file: shared.file,
            language: shared.language,
            source_type: shared.source_type,
            source_map: shared.source_map,
        })
    }
}

impl JsGenerateOptions {
    /// Convert the JS-facing options into the Rust [`GenerateOptions`].
    pub fn to_generate_options(&self) -> Result<GenerateOptions> {
        let source_map: Option<SourceMapOptions> = match &self.source_map {
            | Some(value) => Some(parse_source_map_options(value)?),
            | None => None,
        };

        Ok(GenerateOptions {
            cwd: self.cwd.clone(),
            file: self.file.clone(),
            source_map,
        })
    }
}

impl JsParserOptions {
    /// Convert the JS-facing options into the resolvable form the builtin
    /// parser's options are derived from.
    pub fn to_resolvable(&self) -> Result<ResolvableOptions> {
        let shared: SharedOptions = SharedOptions::parse(
            self.cwd.clone(),
            self.file.clone(),
            self.language.as_deref(),
            self.source_type.as_deref(),
            self.source_map.as_ref(),
        )?;

        Ok(ResolvableOptions {
            cwd: shared.cwd,
            file: shared.file,
            language: shared.language,
            source_type: shared.source_type,
            source_map: shared.source_map,
        })
    }
}

impl JsGeneratorOptions {
    /// Convert the JS-facing options into the resolvable form the builtin
    /// generator's options are derived from. The grammar is inferred from the
    /// resolved file, so no grammar or module-kind field is carried.
    pub fn to_resolvable(&self) -> Result<ResolvableOptions> {
        let source_map: Option<SourceMapOptions> = match &self.source_map {
            | Some(value) => Some(parse_source_map_options(value)?),
            | None => None,
        };

        Ok(ResolvableOptions {
            cwd: self.cwd.clone(),
            file: self.file.clone(),
            language: None,
            source_type: None,
            source_map,
        })
    }
}

#[cfg(test)]
mod tests {
    use telarel_common::{
        InputSourceMapMode, Language, OutputSourceMap, OutputSourceMapMode,
        OutputSourceMapOptions, SourceType,
    };

    use super::*;

    fn options() -> JsParseOptions {
        JsParseOptions {
            cwd: None,
            file: Some(String::from("index.js")),
            language: None,
            source_type: None,
            source_map: None,
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
        assert!(parsed.source_map.is_none());
    }

    #[test]
    fn test_parse_options_keeps_missing_options_none() {
        let parsed: ParseOptions =
            options().to_parse_options().expect("absent options parse");

        assert_eq!(parsed.language, None);
        assert_eq!(parsed.source_type, None);
        assert!(parsed.source_map.is_none());
    }

    #[test]
    fn test_pipeline_options_share_the_same_fields() {
        let parse: ParseOptions =
            options().to_parse_options().expect("parse options parse");

        let compile: CompileOptions = JsCompileOptions {
            cwd: None,
            file: Some(String::from("index.js")),
            language: None,
            source_type: None,
            source_map: None,
            plugins: Vec::new(),
        }
        .to_compile_options()
        .expect("compile options parse");

        let rewrite: RewriteOptions = JsRewriteOptions {
            cwd: None,
            file: Some(String::from("index.js")),
            language: None,
            source_type: None,
            source_map: None,
            plugins: Vec::new(),
        }
        .to_rewrite_options()
        .expect("rewrite options parse");

        let from_ast: CompileFromAstOptions = JsCompileFromAstOptions {
            cwd: None,
            file: Some(String::from("index.js")),
            language: None,
            source_type: None,
            source_map: None,
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

    fn options_with_source_map(value: serde_json::Value) -> JsParseOptions {
        let mut options: JsParseOptions = options();

        options.source_map = Some(value);

        options
    }

    #[test]
    fn test_source_map_omitted_is_off() {
        let parsed: ParseOptions =
            options().to_parse_options().expect("absent sourceMap parses");

        assert!(parsed.source_map.is_none());
    }

    #[test]
    fn test_source_map_empty_object_is_off() {
        let parsed: ParseOptions =
            options_with_source_map(serde_json::json!({}))
                .to_parse_options()
                .expect("empty sourceMap parses");

        let source_map = parsed.source_map.expect("sourceMap is present");

        assert!(source_map.input.is_none());
        assert!(source_map.output.is_none());
    }

    #[test]
    fn test_source_map_input_mode_labels() {
        for (label, expected) in [
            ("inline", InputSourceMapMode::Inline),
            ("external", InputSourceMapMode::External),
            ("auto", InputSourceMapMode::Auto),
        ] {
            let parsed: ParseOptions =
                options_with_source_map(serde_json::json!({ "input": label }))
                    .to_parse_options()
                    .expect("input label parses");

            let input = parsed
                .source_map
                .expect("sourceMap is present")
                .input
                .expect("input is present");

            match input {
                | InputSourceMap::Mode(mode) => {
                    assert!(
                        std::mem::discriminant(&mode)
                            == std::mem::discriminant(&expected),
                        "{label} maps to {mode:?}"
                    );
                },
            }
        }
    }

    #[test]
    fn test_source_map_input_path_string() {
        let parsed: ParseOptions = options_with_source_map(serde_json::json!({
            "input": "maps/app.js.map",
        }))
        .to_parse_options()
        .expect("path string parses");

        let input = parsed
            .source_map
            .expect("sourceMap is present")
            .input
            .expect("input is present");

        match input {
            | InputSourceMap::Mode(InputSourceMapMode::Path(path)) => {
                assert_eq!(path.to_string_lossy(), "maps/app.js.map");
            },
            | other => panic!("expected a path input, got {other:?}"),
        }
    }

    #[test]
    fn test_source_map_input_object_is_value() {
        let parsed: ParseOptions = options_with_source_map(serde_json::json!({
            "input": {
                "version": 3,
                "sources": ["upstream.ts"],
                "mappings": "AAAA",
            },
        }))
        .to_parse_options()
        .expect("object input parses");

        let input = parsed
            .source_map
            .expect("sourceMap is present")
            .input
            .expect("input is present");

        match input {
            | InputSourceMap::Mode(InputSourceMapMode::Value(map)) => {
                let json: oxc_sourcemap::JSONSourceMap = map.to_json();

                assert_eq!(json.sources, vec![String::from("upstream.ts")]);
            },
            | other => panic!("expected a value input, got {other:?}"),
        }
    }

    #[test]
    fn test_source_map_output_mode_string() {
        let parsed: ParseOptions =
            options_with_source_map(serde_json::json!({ "output": "both" }))
                .to_parse_options()
                .expect("output label parses");

        assert!(matches!(
            parsed.source_map.expect("sourceMap is present").output,
            Some(OutputSourceMap::Mode(OutputSourceMapMode::Both))
        ));
    }

    #[test]
    fn test_source_map_output_options_object() {
        let parsed: ParseOptions = options_with_source_map(serde_json::json!({
            "output": { "mode": "inline" },
        }))
        .to_parse_options()
        .expect("output options parse");

        assert!(matches!(
            parsed.source_map.expect("sourceMap is present").output,
            Some(OutputSourceMap::Options(OutputSourceMapOptions {
                mode: OutputSourceMapMode::Inline,
            }))
        ));
    }

    #[test]
    fn test_source_map_input_invalid_shape_errors() {
        let error: napi::Error =
            options_with_source_map(serde_json::json!({ "input": 42 }))
                .to_parse_options()
                .expect_err("invalid input shape errors");

        assert_eq!(error.status, napi::Status::InvalidArg);
        assert!(error.reason.contains("invalid sourceMap.input"));
    }

    #[test]
    fn test_source_map_output_invalid_mode_errors() {
        let error: napi::Error = options_with_source_map(
            serde_json::json!({ "output": "sideways" }),
        )
        .to_parse_options()
        .expect_err("invalid output mode errors");

        assert_eq!(error.status, napi::Status::InvalidArg);
        assert!(error.reason.contains("invalid sourceMap.output mode"));
    }

    #[test]
    fn test_source_map_output_options_without_mode_errors() {
        let error: napi::Error =
            options_with_source_map(serde_json::json!({ "output": {} }))
                .to_parse_options()
                .expect_err("missing mode errors");

        assert_eq!(error.status, napi::Status::InvalidArg);
        assert!(error.reason.contains("requires a `mode`"));
    }

    #[test]
    fn test_source_map_not_an_object_errors() {
        let error: napi::Error =
            options_with_source_map(serde_json::json!("inline"))
                .to_parse_options()
                .expect_err("non-object sourceMap errors");

        assert_eq!(error.status, napi::Status::InvalidArg);
        assert!(error.reason.contains("invalid sourceMap"));
    }

    #[test]
    fn test_generate_options_drop_language_and_source_type() {
        let options: JsGenerateOptions = JsGenerateOptions {
            cwd: Some(String::from("/repo")),
            file: Some(String::from("out.js")),
            source_map: Some(serde_json::json!({ "output": "inline" })),
            plugins: Vec::new(),
        };

        let parsed: GenerateOptions =
            options.to_generate_options().expect("generate options parse");

        assert_eq!(parsed.cwd.as_deref(), Some("/repo"));
        assert_eq!(parsed.file.as_deref(), Some("out.js"));
        assert!(parsed.source_map.is_some());
    }

    #[test]
    fn test_parser_options_convert_to_resolvable() {
        let options: JsParserOptions = JsParserOptions {
            cwd: Some(String::from("/repo")),
            file: Some(String::from("app.tsx")),
            language: Some(String::from("tsx")),
            source_type: Some(String::from("module")),
            source_map: None,
        };

        let resolvable: ResolvableOptions =
            options.to_resolvable().expect("parser options parse");

        assert_eq!(resolvable.cwd.as_deref(), Some("/repo"));
        assert_eq!(resolvable.file.as_deref(), Some("app.tsx"));
        assert_eq!(resolvable.language, Some(Language::TSX));
        assert_eq!(resolvable.source_type, Some(SourceType::Module));
        assert!(resolvable.source_map.is_none());
    }

    #[test]
    fn test_generator_options_convert_to_resolvable_without_grammar() {
        let options: JsGeneratorOptions = JsGeneratorOptions {
            cwd: Some(String::from("/repo")),
            file: Some(String::from("out.js")),
            source_map: None,
        };

        let resolvable: ResolvableOptions =
            options.to_resolvable().expect("generator options parse");

        assert_eq!(resolvable.cwd.as_deref(), Some("/repo"));
        assert_eq!(resolvable.file.as_deref(), Some("out.js"));
        assert_eq!(resolvable.language, None);
        assert_eq!(resolvable.source_type, None);
    }
}
