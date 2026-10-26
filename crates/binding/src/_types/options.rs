use std::path::PathBuf;

use napi::bindgen_prelude::ObjectRef;
use napi::{Error, Result, Status};

use telarel_common::{
    CompileFromAstOptions, CompileOptions, GenerateOptions, InputSourceMap,
    InputSourceMapMode, InputSourceMapOptions, Language, OutputOptions,
    OutputSourceMap, OutputSourceMapMode, OutputSourceMapOptions, ParseOptions,
    ResolvableOptions, RewriteOptions, SourceMap, SourceMapOptions, SourceType,
};

/// JS-facing options for `parse`.
#[napi_derive::napi(object)]
pub struct JsParseOptions {
    /// Current working directory. When the JS side omits it, the value
    /// resolves to the process working directory.
    pub cwd: Option<String>,
    /// The file being parsed. When the JS side omits it, the value resolves
    /// to a default filename.
    pub file: Option<String>,
    /// The grammar of the source. When the JS side omits it, the grammar
    /// comes from the file extension.
    pub language: Option<String>,
    /// The module system of the source. When the JS side omits it, the module
    /// kind stays the one that the grammar resolves.
    #[napi(js_name = "sourceType")]
    pub source_type: Option<String>,
    /// The input source-map request for the step. When the JS side omits it,
    /// Telarel performs no input-map lookup.
    #[napi(js_name = "inputSourceMap")]
    pub input_source_map: Option<serde_json::Value>,
    /// Raw objects for JS plugins, rooted as references.
    /// `JsPlugin::from_object` bridges them.
    pub plugins: Vec<ObjectRef<false>>,
}

/// JS-facing options for output code.
#[napi_derive::napi(object)]
pub struct JsOutputOptions {
    /// The intended path of the generated code.
    pub file: Option<String>,
}

/// JS-facing options for `compile`.
#[napi_derive::napi(object)]
pub struct JsCompileOptions {
    /// Current working directory. When the JS side omits it, the value
    /// resolves to the process working directory.
    pub cwd: Option<String>,
    /// The file being compiled. When the JS side omits it, the value resolves
    /// to a default filename.
    pub file: Option<String>,
    /// The grammar of the source. When the JS side omits it, the grammar
    /// comes from the file extension.
    pub language: Option<String>,
    /// The module system of the source. When the JS side omits it, the module
    /// kind stays the one that the grammar resolves.
    #[napi(js_name = "sourceType")]
    pub source_type: Option<String>,
    /// The output code options.
    pub output: Option<JsOutputOptions>,
    /// The source-map inputs and outputs requested for the step.
    #[napi(js_name = "sourceMap")]
    pub source_map: Option<serde_json::Value>,
    /// Raw objects for JS plugins, rooted as references.
    /// `JsPlugin::from_object` bridges them.
    pub plugins: Vec<ObjectRef<false>>,
}

/// JS-facing options for `rewrite`.
#[napi_derive::napi(object)]
pub struct JsRewriteOptions {
    /// Current working directory. When the JS side omits it, the value
    /// resolves to the process working directory.
    pub cwd: Option<String>,
    /// The file being rewritten. When the JS side omits it, the value
    /// resolves to a default filename.
    pub file: Option<String>,
    /// The grammar of the source. When the JS side omits it, the value falls
    /// back to the AST language, then the file extension.
    pub language: Option<String>,
    /// The module system of the source. When the JS side omits it, the module
    /// kind stays the one that the grammar resolves.
    #[napi(js_name = "sourceType")]
    pub source_type: Option<String>,
    /// The input source-map request for the step. When the JS side omits it,
    /// Telarel performs no input-map lookup.
    #[napi(js_name = "inputSourceMap")]
    pub input_source_map: Option<serde_json::Value>,
    /// Raw objects for JS plugins, rooted as references.
    /// `JsPlugin::from_object` bridges them.
    pub plugins: Vec<ObjectRef<false>>,
}

/// JS-facing options for `compileFromAst`.
#[napi_derive::napi(object)]
pub struct JsCompileFromAstOptions {
    /// Current working directory. When the JS side omits it, the value
    /// resolves to the process working directory.
    pub cwd: Option<String>,
    /// The file being compiled. When the JS side omits it, the value resolves
    /// to a default filename.
    pub file: Option<String>,
    /// The grammar of the source. When the JS side omits it, the value falls
    /// back to the AST language, then the file extension.
    pub language: Option<String>,
    /// The module system of the source. When the JS side omits it, the module
    /// kind stays the one that the grammar resolves.
    #[napi(js_name = "sourceType")]
    pub source_type: Option<String>,
    /// The output code options.
    pub output: Option<JsOutputOptions>,
    /// The source-map inputs and outputs requested for the step.
    #[napi(js_name = "sourceMap")]
    pub source_map: Option<serde_json::Value>,
    /// Raw objects for JS plugins, rooted as references.
    /// `JsPlugin::from_object` bridges them.
    pub plugins: Vec<ObjectRef<false>>,
}

/// JS-facing options for `generate`. The module kind is absent because the
/// generated program's own `sourceType` settles it.
#[napi_derive::napi(object)]
pub struct JsGenerateOptions {
    /// Current working directory. When the JS side omits it, the value
    /// resolves to the process working directory.
    pub cwd: Option<String>,
    /// The file being generated. When the JS side omits it, the value
    /// resolves to a default filename.
    pub file: Option<String>,
    /// The grammar of the generated code. When the JS side omits it, the
    /// value falls back to the AST language, then the file extension.
    pub language: Option<String>,
    /// The output code options.
    pub output: Option<JsOutputOptions>,
    /// The source-map inputs and outputs requested for the step.
    #[napi(js_name = "sourceMap")]
    pub source_map: Option<serde_json::Value>,
    /// Raw objects for JS plugins, rooted as references.
    /// `JsPlugin::from_object` bridges them.
    pub plugins: Vec<ObjectRef<false>>,
}

/// JS-facing options for `builtinParse` (the builtin `createParser`). There
/// are no plugins, because the builtin parser never runs the plugin chain.
#[napi_derive::napi(object)]
pub struct JsParserOptions {
    /// Current working directory. When the JS side omits it, the value
    /// resolves to the process working directory.
    pub cwd: Option<String>,
    /// The file being parsed. When the JS side omits it, the value resolves
    /// to a default filename.
    pub file: Option<String>,
    /// The grammar of the source. When the JS side omits it, the grammar
    /// comes from the file extension.
    pub language: Option<String>,
    /// The module system of the source. When the JS side omits it, the module
    /// kind stays the one that the grammar resolves.
    #[napi(js_name = "sourceType")]
    pub source_type: Option<String>,
    /// The source-map inputs and outputs requested for the step.
    #[napi(js_name = "sourceMap")]
    pub source_map: Option<serde_json::Value>,
}

/// JS-facing options for `builtinGenerate` (the builtin `createGenerator`).
/// There are no plugins, no grammar, and no module kind.
#[napi_derive::napi(object)]
pub struct JsGeneratorOptions {
    /// Current working directory. When the JS side omits it, the value
    /// resolves to the process working directory.
    pub cwd: Option<String>,
    /// The file being generated. When the JS side omits it, the value
    /// resolves to a default filename.
    pub file: Option<String>,
    /// The source-map inputs and outputs requested for the step.
    #[napi(js_name = "sourceMap")]
    pub source_map: Option<serde_json::Value>,
}

fn invalid_arg(message: String) -> Error {
    Error::new(Status::InvalidArg, message)
}

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

pub(crate) fn parse_language_option(
    language: Option<&str>
) -> Result<Option<Language>> {
    match language {
        | Some(text) => {
            Ok(Some(text.parse::<Language>().map_err(invalid_arg)?))
        },
        | None => Ok(None),
    }
}

fn parse_source_type_option(
    source_type: Option<&str>
) -> Result<Option<SourceType>> {
    match source_type {
        | Some(text) => Ok(Some(parse_source_type(text).map_err(invalid_arg)?)),
        | None => Ok(None),
    }
}

fn parse_source_map_object(
    value: &serde_json::Value,
    label: &str,
) -> Result<SourceMap> {
    let json: String = serde_json::to_string(value).map_err(|error| {
        invalid_arg(format!("invalid {label} object: {error}"))
    })?;

    oxc_sourcemap::OwnedSourceMap::from_json_string(&json)
        .map(oxc_sourcemap::OwnedSourceMap::into_inner)
        .map_err(|error| {
            invalid_arg(format!("invalid {label} object: {error}"))
        })
}

/// Parse an optional string field that must not be empty.
fn parse_optional_string<'a>(
    object: &'a serde_json::Map<String, serde_json::Value>,
    key: &str,
    label: &str,
) -> Result<Option<&'a str>> {
    match object.get(key) {
        | Some(serde_json::Value::Null) | None => Ok(None),
        | Some(serde_json::Value::String(text)) if !text.is_empty() => {
            Ok(Some(text))
        },
        | Some(_) => Err(invalid_arg(format!(
            "invalid {label}: expected a non-empty string"
        ))),
    }
}

/// Parse an optional `url` field for output source maps.
///
/// An absent, `null`, or empty-string URL is unset, matching the resolved
/// contract. Any other non-string value is invalid.
fn parse_optional_url<'a>(
    object: &'a serde_json::Map<String, serde_json::Value>,
    key: &str,
    label: &str,
) -> Result<Option<&'a str>> {
    match object.get(key) {
        | Some(serde_json::Value::Null) | None => Ok(None),
        | Some(serde_json::Value::String(text)) if !text.is_empty() => {
            Ok(Some(text))
        },
        | Some(serde_json::Value::String(_)) => Ok(None),
        | Some(_) => {
            Err(invalid_arg(format!("invalid {label}: expected a string")))
        },
    }
}

fn parse_input_source_map_mode(
    text: &str,
    label: &str,
) -> Result<InputSourceMapMode> {
    match text {
        | "inline" => Ok(InputSourceMapMode::Inline),
        | "external" => Ok(InputSourceMapMode::External),
        | "auto" => Ok(InputSourceMapMode::Auto),
        | _ => Err(invalid_arg(format!(
            "invalid {label}: expected \"inline\", \"external\" or \"auto\""
        ))),
    }
}

fn parse_input_source_map_mode_value(
    value: &serde_json::Value,
    label: &str,
) -> Result<InputSourceMapMode> {
    match value {
        | serde_json::Value::String(text) => {
            parse_input_source_map_mode(text.as_str(), label)
        },
        | serde_json::Value::Object(_) => Ok(InputSourceMapMode::Value(
            parse_source_map_object(value, label)?,
        )),
        | _ => Err(invalid_arg(format!(
            "invalid {label}: expected a mode string or a source map object"
        ))),
    }
}

fn parse_input_source_map_options(
    object: &serde_json::Map<String, serde_json::Value>,
    label: &str,
) -> Result<InputSourceMap> {
    let mode: &serde_json::Value = object.get("mode").ok_or_else(|| {
        invalid_arg(format!(
            "invalid {label}: an options object requires a `mode`"
        ))
    })?;

    let mode: InputSourceMapMode =
        parse_input_source_map_mode_value(mode, &format!("{label}.mode"))?;

    let path: Option<PathBuf> =
        parse_optional_string(object, "path", &format!("{label}.path"))?
            .map(PathBuf::from);

    Ok(InputSourceMap::Options(InputSourceMapOptions { mode, path }))
}

fn parse_input_source_map(
    value: &serde_json::Value,
    label: &str,
) -> Result<InputSourceMap> {
    match value {
        | serde_json::Value::Object(object) => {
            if object.contains_key("mode") {
                parse_input_source_map_options(object, label)
            } else if object.contains_key("path") {
                Err(invalid_arg(format!(
                    "invalid {label}: an options object requires a `mode`"
                )))
            } else {
                Ok(InputSourceMap::Mode(InputSourceMapMode::Value(
                    parse_source_map_object(value, label)?,
                )))
            }
        },
        | serde_json::Value::String(text) => {
            let mode: InputSourceMapMode =
                parse_input_source_map_mode(text.as_str(), label)?;

            Ok(InputSourceMap::Mode(mode))
        },
        | _ => Err(invalid_arg(format!(
            "invalid {label}: expected a source map object, an options object, \
             or \"inline\" / \"external\" / \"auto\""
        ))),
    }
}

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

            let url: Option<String> =
                parse_optional_url(object, "url", "sourceMap.output.url")?
                    .map(str::to_owned);

            Ok(OutputSourceMap::Options(OutputSourceMapOptions {
                mode: parse_output_source_map_mode(text)?,
                url,
            }))
        },
        | _ => Err(invalid_arg(
            "invalid sourceMap.output: expected \"inline\" / \"external\" / \
             \"both\" or { mode, url? }"
                .to_owned(),
        )),
    }
}

/// Parse the JS `sourceMap` union into the Rust option family.
///
/// `null` and absent keys are "off" (`None`), never `"auto"`. A malformed
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
        | Some(value) => {
            Some(parse_input_source_map(value, "sourceMap.input")?)
        },
    };

    let output: Option<OutputSourceMap> = match object.get("output") {
        | Some(serde_json::Value::Null) | None => None,
        | Some(value) => Some(parse_output_source_map(value)?),
    };

    Ok(SourceMapOptions { input, output })
}

pub fn parse_output_options(
    output: Option<&JsOutputOptions>
) -> Result<Option<OutputOptions>> {
    match output {
        | None => Ok(None),
        | Some(output) => {
            let file: Option<String> = match output.file.as_deref() {
                | Some("") | None => None,
                | Some(file) => Some(file.to_owned()),
            };

            Ok(Some(OutputOptions { file }))
        },
    }
}

/// The shared scalar fields of an entry-point option struct, after parsing.
///
/// `compile` and `compileFromAst` carry these six scalars on the wire. The
/// builtin parser options carry all of them except `output`. Each entry point
/// bridges its own plugins array.
struct SharedOptions {
    cwd: Option<PathBuf>,
    file: Option<String>,
    language: Option<Language>,
    source_type: Option<SourceType>,
    output: Option<OutputOptions>,
    source_map: Option<SourceMapOptions>,
}

impl SharedOptions {
    fn parse(
        cwd: Option<String>,
        file: Option<String>,
        language: Option<&str>,
        source_type: Option<&str>,
        output: Option<&JsOutputOptions>,
        source_map: Option<&serde_json::Value>,
    ) -> Result<Self> {
        let language: Option<Language> = parse_language_option(language)?;
        let source_type: Option<SourceType> =
            parse_source_type_option(source_type)?;

        let output: Option<OutputOptions> = parse_output_options(output)?;

        let source_map: Option<SourceMapOptions> = match source_map {
            | Some(value) => Some(parse_source_map_options(value)?),
            | None => None,
        };

        Ok(Self {
            cwd: cwd.map(PathBuf::from),
            file,
            language,
            source_type,
            output,
            source_map,
        })
    }
}

/// The parsed scalar fields of an entry-point option struct.
///
/// `parse` and `rewrite` carry the shared scalars plus a flat `inputSourceMap`
/// request. Output maps do not apply to AST-producing entry points.
struct SharedInputOptions {
    cwd: Option<PathBuf>,
    file: Option<String>,
    language: Option<Language>,
    source_type: Option<SourceType>,
    input_source_map: Option<InputSourceMap>,
}

impl SharedInputOptions {
    fn parse(
        cwd: Option<String>,
        file: Option<String>,
        language: Option<&str>,
        source_type: Option<&str>,
        input_source_map: Option<&serde_json::Value>,
    ) -> Result<Self> {
        let language: Option<Language> = parse_language_option(language)?;
        let source_type: Option<SourceType> =
            parse_source_type_option(source_type)?;

        let input_source_map: Option<InputSourceMap> = match input_source_map {
            | Some(value) => {
                Some(parse_input_source_map(value, "inputSourceMap")?)
            },
            | None => None,
        };

        Ok(Self {
            cwd: cwd.map(PathBuf::from),
            file,
            language,
            source_type,
            input_source_map,
        })
    }
}

impl JsParseOptions {
    /// Convert the JS-facing options into the Rust [`ParseOptions`]. Invalid
    /// labels or malformed source maps return `InvalidArg`.
    pub fn to_parse_options(&self) -> Result<ParseOptions> {
        let shared: SharedInputOptions = SharedInputOptions::parse(
            self.cwd.clone(),
            self.file.clone(),
            self.language.as_deref(),
            self.source_type.as_deref(),
            self.input_source_map.as_ref(),
        )?;

        Ok(ParseOptions {
            cwd: shared.cwd,
            file: shared.file,
            language: shared.language,
            source_type: shared.source_type,
            input_source_map: shared.input_source_map,
        })
    }
}

impl JsCompileOptions {
    /// Convert the JS-facing options into the Rust [`CompileOptions`]. Invalid
    /// labels or malformed source maps return `InvalidArg`.
    pub fn to_compile_options(&self) -> Result<CompileOptions> {
        let shared: SharedOptions = SharedOptions::parse(
            self.cwd.clone(),
            self.file.clone(),
            self.language.as_deref(),
            self.source_type.as_deref(),
            self.output.as_ref(),
            self.source_map.as_ref(),
        )?;

        Ok(CompileOptions {
            cwd: shared.cwd,
            file: shared.file,
            language: shared.language,
            source_type: shared.source_type,
            output: shared.output,
            source_map: shared.source_map,
        })
    }
}

impl JsRewriteOptions {
    /// Convert the JS-facing options into the Rust [`RewriteOptions`]. Invalid
    /// labels or malformed source maps return `InvalidArg`.
    pub fn to_rewrite_options(&self) -> Result<RewriteOptions> {
        let shared: SharedInputOptions = SharedInputOptions::parse(
            self.cwd.clone(),
            self.file.clone(),
            self.language.as_deref(),
            self.source_type.as_deref(),
            self.input_source_map.as_ref(),
        )?;

        Ok(RewriteOptions {
            cwd: shared.cwd,
            file: shared.file,
            language: shared.language,
            source_type: shared.source_type,
            input_source_map: shared.input_source_map,
        })
    }
}

impl JsCompileFromAstOptions {
    /// Convert the JS-facing options into the Rust [`CompileFromAstOptions`].
    /// Invalid labels or malformed source maps return `InvalidArg`.
    pub fn to_compile_from_ast_options(&self) -> Result<CompileFromAstOptions> {
        let shared: SharedOptions = SharedOptions::parse(
            self.cwd.clone(),
            self.file.clone(),
            self.language.as_deref(),
            self.source_type.as_deref(),
            self.output.as_ref(),
            self.source_map.as_ref(),
        )?;

        Ok(CompileFromAstOptions {
            cwd: shared.cwd,
            file: shared.file,
            language: shared.language,
            source_type: shared.source_type,
            output: shared.output,
            source_map: shared.source_map,
        })
    }
}

impl JsGenerateOptions {
    /// Convert the JS-facing options into the Rust [`GenerateOptions`]. Invalid
    /// labels or malformed source maps return `InvalidArg`.
    pub fn to_generate_options(&self) -> Result<GenerateOptions> {
        let language: Option<Language> =
            parse_language_option(self.language.as_deref())?;

        let source_map: Option<SourceMapOptions> = match &self.source_map {
            | Some(value) => Some(parse_source_map_options(value)?),
            | None => None,
        };

        Ok(GenerateOptions {
            cwd: self.cwd.clone().map(PathBuf::from),
            file: self.file.clone(),
            language,
            output: parse_output_options(self.output.as_ref())?,
            source_map,
        })
    }
}

impl JsParserOptions {
    /// Convert the JS-facing options into the resolvable form for the builtin
    /// parser. The builtin parser derives its options from that form. Invalid
    /// labels or malformed source maps return `InvalidArg`.
    pub fn to_resolvable(&self) -> Result<ResolvableOptions> {
        let shared: SharedOptions = SharedOptions::parse(
            self.cwd.clone(),
            self.file.clone(),
            self.language.as_deref(),
            self.source_type.as_deref(),
            None,
            self.source_map.as_ref(),
        )?;

        Ok(ResolvableOptions {
            cwd: shared.cwd,
            file: shared.file,
            language: shared.language,
            source_type: shared.source_type,
            output: None,
            source_map: shared.source_map,
        })
    }
}

impl JsGeneratorOptions {
    /// Convert the JS-facing options into the resolvable form for the builtin
    /// generator. The form carries no grammar or module-kind field. The
    /// builtin generator resolves the grammar from the input AST's language,
    /// then from the resolved file. Invalid labels or malformed source maps
    /// return `InvalidArg`.
    pub fn to_resolvable(&self) -> Result<ResolvableOptions> {
        let source_map: Option<SourceMapOptions> = match &self.source_map {
            | Some(value) => Some(parse_source_map_options(value)?),
            | None => None,
        };

        Ok(ResolvableOptions {
            cwd: self.cwd.clone().map(PathBuf::from),
            file: self.file.clone(),
            language: None,
            source_type: None,
            output: None,
            source_map,
        })
    }
}

#[cfg(test)]
mod tests {
    use telarel_common::{
        InputSourceMapMode, InputSourceMapOptions, Language, OutputOptions,
        OutputSourceMap, OutputSourceMapMode, OutputSourceMapOptions,
        SourceType,
    };

    use super::*;

    fn options() -> JsParseOptions {
        JsParseOptions {
            cwd: None,
            file: Some(String::from("index.js")),
            language: None,
            source_type: None,
            input_source_map: None,
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

    fn options_with_input_source_map(
        value: serde_json::Value
    ) -> JsParseOptions {
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
        let error: napi::Error = options_with_input_source_map(
            serde_json::json!({ "path": "a.map" }),
        )
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

                assert_eq!(json.sources, vec![String::from("upstream.ts")]);
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

                assert_eq!(json.sources, vec![String::from("upstream.ts")]);

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
            let error: napi::Error = options_with_input_source_map(
                serde_json::json!({ "mode": mode }),
            )
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

        let parsed: RewriteOptions = options
            .to_rewrite_options()
            .expect("rewrite inputSourceMap parses");

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
        let error: napi::Error = compile_options_with_source_map(
            serde_json::json!({ "output": {} }),
        )
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
            plugins: Vec::new(),
        };

        let parsed: CompileOptions =
            options.to_compile_options().expect("compile options parse");

        assert_eq!(parsed.cwd, Some(PathBuf::from("/repo")));
        assert!(parsed.source_map.is_none());
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

        assert_eq!(resolvable.cwd, Some(PathBuf::from("/repo")));
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

        assert_eq!(resolvable.cwd, Some(PathBuf::from("/repo")));
        assert_eq!(resolvable.file.as_deref(), Some("out.js"));
        assert_eq!(resolvable.language, None);
        assert_eq!(resolvable.source_type, None);
    }
}
