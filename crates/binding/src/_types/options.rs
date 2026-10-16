use std::path::PathBuf;

use napi::bindgen_prelude::ObjectRef;
use napi::{Error, Result, Status};

use telarel_common::{
    CompileOptions, InputSourceMap, InputSourceMapMode, Language,
    OutputSourceMap, OutputSourceMapMode, OutputSourceMapOptions, SourceMap,
    SourceMapOptions, SourceType,
};

/// JS-facing compile options.
///
/// TEMPORARY ADAPTER (Task-4 deletion target): keeps the old
/// `compile(options)` JS surface alive for one more task while the Rust API
/// moves to `compile(code, options, plugins)`. Task 4 replaces this shape with
/// the real JS surface.
#[napi_derive::napi(object)]
pub struct JsOptions {
    /// Current working directory;
    /// omitted on the JS side resolves to the process working directory.
    pub cwd: Option<String>,
    /// The file to compile.
    pub file: String,
    /// The source code to compile;
    /// the adapter lifts it into `core::compile`'s first argument.
    pub code: String,
    /// The grammar of the code;
    /// omitted on the JS side infers the grammar from the file extension.
    pub language: Option<String>,
    /// The module system of the code;
    /// omitted on the JS side keeps the module kind resolved from the grammar.
    #[napi(js_name = "sourceType")]
    pub source_type: Option<String>,
    /// The source-map inputs and outputs.
    ///
    /// The union exists on the wire; the JS wrapper does not advertise it
    /// until Task 4 but must not choke on it. Omitted means "off", never
    /// `"auto"`.
    #[napi(js_name = "sourceMap")]
    pub source_map: Option<serde_json::Value>,
    /// Raw JS plugin objects, rooted as references;
    /// bridged by `JsPlugin::from_object`.
    pub plugins: Vec<ObjectRef<false>>,
}

/// Parse a JS-facing language string into the Rust enum.
fn parse_language(text: &str) -> std::result::Result<Language, String> {
    match text {
        | "js" => Ok(Language::JS),
        | "ts" => Ok(Language::TS),
        | "dts" => Ok(Language::DTS),
        | "jsx" => Ok(Language::JSX),
        | "tsx" => Ok(Language::TSX),
        | _ => Err(format!(
            "invalid language {text:?}: expected \"js\", \"ts\", \"dts\", \"jsx\" or \"tsx\""
        )),
    }
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
        Error::new(
            Status::InvalidArg,
            format!("invalid sourceMap.input object: {error}"),
        )
    })?;

    oxc_sourcemap::OwnedSourceMap::from_json_string(&json)
        .map(oxc_sourcemap::OwnedSourceMap::into_inner)
        .map_err(|error| {
            Error::new(
                Status::InvalidArg,
                format!("invalid sourceMap.input object: {error}"),
            )
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
        | _ => Err(Error::new(
            Status::InvalidArg,
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
        | _ => Err(Error::new(
            Status::InvalidArg,
            format!(
                "invalid sourceMap.output mode {text:?}: expected \"inline\", \
                 \"external\" or \"both\""
            ),
        )),
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
                    Error::new(
                    Status::InvalidArg,
                    "invalid sourceMap.output: an options object requires a \
                     `mode`"
                        .to_owned(),
                )
                })?;

            let text: &str = mode.as_str().ok_or_else(|| {
                Error::new(
                    Status::InvalidArg,
                    "invalid sourceMap.output.mode: expected a string"
                        .to_owned(),
                )
            })?;

            Ok(OutputSourceMap::Options(OutputSourceMapOptions {
                mode: parse_output_source_map_mode(text)?,
            }))
        },
        | _ => Err(Error::new(
            Status::InvalidArg,
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
pub(crate) fn parse_source_map_options(
    value: &serde_json::Value
) -> Result<SourceMapOptions> {
    let object: &serde_json::Map<String, serde_json::Value> =
        value.as_object().ok_or_else(|| {
            Error::new(
                Status::InvalidArg,
                "invalid sourceMap: expected an object".to_owned(),
            )
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

/// Convert JS options into Rust options (plugins are bridged separately).
///
/// TEMPORARY ADAPTER (Task-4 deletion target): `code` is deliberately NOT
/// copied here; the binding lifts it into `core::compile`'s first argument.
pub fn to_compile_options(options: &JsOptions) -> Result<CompileOptions> {
    let language: Option<Language> = match &options.language {
        | Some(text) => {
            Some(parse_language(text).map_err(|message: String| {
                Error::new(Status::InvalidArg, message)
            })?)
        },
        | None => None,
    };

    let source_type: Option<SourceType> = match &options.source_type {
        | Some(text) => {
            Some(parse_source_type(text).map_err(|message: String| {
                Error::new(Status::InvalidArg, message)
            })?)
        },
        | None => None,
    };

    let source_map: Option<SourceMapOptions> = match &options.source_map {
        | Some(value) => Some(parse_source_map_options(value)?),
        | None => None,
    };

    Ok(CompileOptions {
        cwd: options.cwd.clone(),
        file: Some(options.file.clone()),
        language,
        source_type,
        source_map,
    })
}

#[cfg(test)]
mod tests {
    use telarel_common::{
        InputSourceMapMode, Language, OutputSourceMap, OutputSourceMapMode,
        OutputSourceMapOptions, SourceType,
    };

    use super::{JsOptions, to_compile_options};

    fn fixtures(
        language: Option<String>,
        source_type: Option<String>,
    ) -> JsOptions {
        JsOptions {
            cwd: None,
            file: String::from("index.js"),
            code: String::from("const a = 1;"),
            language,
            source_type,
            source_map: None,
            plugins: Vec::new(),
        }
    }

    #[test]
    fn test_parse_language_valid_values() {
        let expected: [(&str, Language); 5] = [
            ("js", Language::JS),
            ("ts", Language::TS),
            ("dts", Language::DTS),
            ("jsx", Language::JSX),
            ("tsx", Language::TSX),
        ];

        for (text, language) in expected {
            assert_eq!(super::parse_language(text), Ok(language), "{text}");
        }
    }

    #[test]
    fn test_parse_language_invalid_value_errors() {
        for text in ["", "txt", "json", "TS", "Js", "d.ts"] {
            assert!(super::parse_language(text).is_err(), "{text} must error");
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
            assert_eq!(
                super::parse_source_type(text),
                Ok(source_type),
                "{text}"
            );
        }
    }

    #[test]
    fn test_parse_source_type_invalid_value_errors() {
        for text in ["", "js", "ts", "Module", "commonJS"] {
            assert!(
                super::parse_source_type(text).is_err(),
                "{text} must error"
            );
        }
    }

    #[test]
    fn test_to_compile_options_maps_valid_values() {
        let options: JsOptions =
            fixtures(Some(String::from("tsx")), Some(String::from("commonjs")));

        let parsed: telarel_common::CompileOptions =
            to_compile_options(&options).expect("valid options parse");

        assert_eq!(parsed.language, Some(Language::TSX));
        assert_eq!(parsed.source_type, Some(SourceType::CommonJS));
        assert_eq!(parsed.file.as_deref(), Some("index.js"));
        assert_eq!(parsed.cwd, None);
        assert!(parsed.source_map.is_none());
    }

    #[test]
    fn test_to_compile_options_keeps_missing_options_none() {
        let options: JsOptions = fixtures(None, None);

        let parsed: telarel_common::CompileOptions =
            to_compile_options(&options).expect("absent options parse");

        assert_eq!(parsed.language, None);
        assert_eq!(parsed.source_type, None);
        assert!(parsed.source_map.is_none());
    }

    #[test]
    fn test_to_compile_options_invalid_language_errors() {
        let options: JsOptions = fixtures(Some(String::from("json")), None);

        let error: napi::Error =
            to_compile_options(&options).expect_err("invalid language errors");

        assert_eq!(error.status, napi::Status::InvalidArg);
        assert!(error.reason.contains("invalid language \"json\""));
    }

    #[test]
    fn test_to_compile_options_invalid_source_type_errors() {
        let options: JsOptions = fixtures(None, Some(String::from("js")));

        let error: napi::Error = to_compile_options(&options)
            .expect_err("invalid source type errors");

        assert_eq!(error.status, napi::Status::InvalidArg);
        assert!(error.reason.contains("invalid source type \"js\""));
    }

    fn options_with_source_map(value: serde_json::Value) -> JsOptions {
        let mut options: JsOptions = fixtures(None, None);

        options.source_map = Some(value);

        options
    }

    #[test]
    fn test_source_map_omitted_is_off() {
        let options: JsOptions = fixtures(None, None);

        let parsed: telarel_common::CompileOptions =
            to_compile_options(&options).expect("absent sourceMap parses");

        assert!(parsed.source_map.is_none());
    }

    #[test]
    fn test_source_map_empty_object_is_off() {
        let options: JsOptions = options_with_source_map(serde_json::json!({}));

        let parsed: telarel_common::CompileOptions =
            to_compile_options(&options).expect("empty sourceMap parses");

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
            let options: JsOptions =
                options_with_source_map(serde_json::json!({
                    "input": label,
                }));

            let parsed: telarel_common::CompileOptions =
                to_compile_options(&options).expect("input label parses");

            let input = parsed
                .source_map
                .expect("sourceMap is present")
                .input
                .expect("input is present");

            match input {
                | telarel_common::InputSourceMap::Mode(mode) => {
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
        let options: JsOptions = options_with_source_map(serde_json::json!({
            "input": "maps/app.js.map",
        }));

        let parsed: telarel_common::CompileOptions =
            to_compile_options(&options).expect("path string parses");

        let input = parsed
            .source_map
            .expect("sourceMap is present")
            .input
            .expect("input is present");

        match input {
            | telarel_common::InputSourceMap::Mode(
                InputSourceMapMode::Path(path),
            ) => {
                assert_eq!(path.to_string_lossy(), "maps/app.js.map");
            },
            | other => panic!("expected a path input, got {other:?}"),
        }
    }

    #[test]
    fn test_source_map_input_object_is_value() {
        let options: JsOptions = options_with_source_map(serde_json::json!({
            "input": {
                "version": 3,
                "sources": ["upstream.ts"],
                "mappings": "AAAA",
            },
        }));

        let parsed: telarel_common::CompileOptions =
            to_compile_options(&options).expect("object input parses");

        let input = parsed
            .source_map
            .expect("sourceMap is present")
            .input
            .expect("input is present");

        match input {
            | telarel_common::InputSourceMap::Mode(
                InputSourceMapMode::Value(map),
            ) => {
                let json: oxc_sourcemap::JSONSourceMap = map.to_json();

                assert_eq!(json.sources, vec![String::from("upstream.ts")]);
            },
            | other => panic!("expected a value input, got {other:?}"),
        }
    }

    #[test]
    fn test_source_map_output_mode_string() {
        let options: JsOptions = options_with_source_map(serde_json::json!({
            "output": "both",
        }));

        let parsed: telarel_common::CompileOptions =
            to_compile_options(&options).expect("output label parses");

        assert!(matches!(
            parsed.source_map.expect("sourceMap is present").output,
            Some(OutputSourceMap::Mode(OutputSourceMapMode::Both))
        ));
    }

    #[test]
    fn test_source_map_output_options_object() {
        let options: JsOptions = options_with_source_map(serde_json::json!({
            "output": { "mode": "inline" },
        }));

        let parsed: telarel_common::CompileOptions =
            to_compile_options(&options).expect("output options parse");

        assert!(matches!(
            parsed.source_map.expect("sourceMap is present").output,
            Some(OutputSourceMap::Options(OutputSourceMapOptions {
                mode: OutputSourceMapMode::Inline,
            }))
        ));
    }

    #[test]
    fn test_source_map_input_invalid_shape_errors() {
        let options: JsOptions = options_with_source_map(serde_json::json!({
            "input": 42,
        }));

        let error: napi::Error = to_compile_options(&options)
            .expect_err("invalid input shape errors");

        assert_eq!(error.status, napi::Status::InvalidArg);
        assert!(error.reason.contains("invalid sourceMap.input"));
    }

    #[test]
    fn test_source_map_output_invalid_mode_errors() {
        let options: JsOptions = options_with_source_map(serde_json::json!({
            "output": "sideways",
        }));

        let error: napi::Error = to_compile_options(&options)
            .expect_err("invalid output mode errors");

        assert_eq!(error.status, napi::Status::InvalidArg);
        assert!(error.reason.contains("invalid sourceMap.output mode"));
    }

    #[test]
    fn test_source_map_output_options_without_mode_errors() {
        let options: JsOptions = options_with_source_map(serde_json::json!({
            "output": {},
        }));

        let error: napi::Error =
            to_compile_options(&options).expect_err("missing mode errors");

        assert_eq!(error.status, napi::Status::InvalidArg);
        assert!(error.reason.contains("requires a `mode`"));
    }

    #[test]
    fn test_source_map_not_an_object_errors() {
        let options: JsOptions =
            options_with_source_map(serde_json::json!("inline"));

        let error: napi::Error = to_compile_options(&options)
            .expect_err("non-object sourceMap errors");

        assert_eq!(error.status, napi::Status::InvalidArg);
        assert!(error.reason.contains("invalid sourceMap"));
    }
}
