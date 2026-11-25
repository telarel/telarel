use std::path::PathBuf;

use napi::bindgen_prelude::ObjectRef;
use napi::{Error, Result, Status};
use oxc::codegen::{CommentOptions, IndentChar, LegalComment};

use telarel_common::{
    CompileFromAstOptions, CompileOptions, GenerateOptions, GeneratorOptions,
    IndentStyle, InputSourceMap, InputSourceMapMode, InputSourceMapOptions,
    Language, OutputOptions, OutputSourceMap, OutputSourceMapMode,
    OutputSourceMapOptions, OxcCodegenOptions, OxcParseOptions, ParseOptions,
    ParserOptions, PartialOxcCodegenOptions, PartialOxcCommentOptions,
    PartialOxcParseOptions, ResolvableOptions, RewriteOptions, SourceMap,
    SourceMapOptions, SourceType,
};

/// JS-facing options for `parse`.
#[napi_derive::napi(object)]
pub struct JsParseOptions {
    /// Current working directory.
    ///
    /// By default, it is the process working directory.
    pub cwd: Option<String>,
    /// The file being parsed.
    ///
    /// By default, it is `"index.ts"`.
    pub file: Option<String>,
    /// The grammar of the source.
    ///
    /// By default, it is inferred from the file extension.
    pub language: Option<String>,
    /// The module system of the source.
    ///
    /// By default, it is resolved from the grammar.
    #[napi(js_name = "sourceType")]
    pub source_type: Option<String>,
    /// Parser-stage options.
    ///
    /// By default, it is `None`, which applies the `ParserOptions` defaults.
    pub parser: Option<JsParserOptions>,
    /// The input source-map request for the step.
    ///
    /// By default, it is `None`, which disables input-map lookup.
    #[napi(js_name = "inputSourceMap")]
    pub input_source_map: Option<serde_json::Value>,
    /// Raw objects for JS plugins, rooted as references.
    /// `JsPlugin::from_object` bridges them.
    ///
    /// By default, it is `[]`.
    pub plugins: Vec<ObjectRef<false>>,
}

/// JS-facing options for output code.
#[napi_derive::napi(object)]
pub struct JsOutputOptions {
    /// The intended path of the generated code. A non-empty value is used
    /// verbatim. An empty value is unset. The transform plugin may set
    /// the file in its options hook; otherwise the compiler uses the resolved
    /// input `file`. The resolved value is recorded as `map.file` and derives
    /// the default external source-map URL.
    ///
    /// By default, it is `None`, which derives the output file from the input
    /// file.
    pub file: Option<String>,
}

/// JS-facing options for `compile`.
#[napi_derive::napi(object)]
pub struct JsCompileOptions {
    /// Current working directory.
    ///
    /// By default, it is the process working directory.
    pub cwd: Option<String>,
    /// The file being compiled.
    ///
    /// By default, it is `"index.ts"`.
    pub file: Option<String>,
    /// The grammar of the source.
    ///
    /// By default, it is inferred from the file extension.
    pub language: Option<String>,
    /// The module system of the source.
    ///
    /// By default, it is resolved from the grammar.
    #[napi(js_name = "sourceType")]
    pub source_type: Option<String>,
    /// Parser-stage options.
    ///
    /// By default, it is `None`, which applies the `ParserOptions` defaults.
    pub parser: Option<JsParserOptions>,
    /// Generator-stage options.
    ///
    /// By default, it is `None`, which applies the `GeneratorOptions` defaults.
    pub generator: Option<JsGeneratorOptions>,
    /// The output code options.
    ///
    /// By default, it is `None`.
    pub output: Option<JsOutputOptions>,
    /// The source-map inputs and outputs requested for the step.
    ///
    /// By default, it is `None`, which disables source-map input lookup and
    /// output delivery.
    #[napi(js_name = "sourceMap")]
    pub source_map: Option<serde_json::Value>,
    /// Raw objects for JS plugins, rooted as references.
    /// `JsPlugin::from_object` bridges them.
    ///
    /// By default, it is `[]`.
    pub plugins: Vec<ObjectRef<false>>,
}

/// JS-facing options for `rewrite`.
#[napi_derive::napi(object)]
pub struct JsRewriteOptions {
    /// Current working directory.
    ///
    /// By default, it is the process working directory.
    pub cwd: Option<String>,
    /// The file being rewritten.
    ///
    /// By default, it is `"index.ts"`.
    pub file: Option<String>,
    /// The grammar of the source.
    ///
    /// By default, it is taken from the input AST grammar, then inferred from
    /// the file extension.
    pub language: Option<String>,
    /// The module system of the source.
    ///
    /// By default, it is taken from the input AST module kind, then resolved
    /// from the file extension.
    #[napi(js_name = "sourceType")]
    pub source_type: Option<String>,
    /// The input source-map request for the step.
    ///
    /// By default, it is `None`, which disables input-map lookup.
    #[napi(js_name = "inputSourceMap")]
    pub input_source_map: Option<serde_json::Value>,
    /// Raw objects for JS plugins, rooted as references.
    /// `JsPlugin::from_object` bridges them.
    ///
    /// By default, it is `[]`.
    pub plugins: Vec<ObjectRef<false>>,
}

/// JS-facing options for `compileFromAst`.
#[napi_derive::napi(object)]
pub struct JsCompileFromAstOptions {
    /// Current working directory.
    ///
    /// By default, it is the process working directory.
    pub cwd: Option<String>,
    /// The file being compiled.
    ///
    /// By default, it is `"index.ts"`.
    pub file: Option<String>,
    /// The grammar of the source.
    ///
    /// By default, it is taken from the input AST grammar, then inferred from
    /// the file extension.
    pub language: Option<String>,
    /// The module system of the source.
    ///
    /// By default, it is taken from the input AST module kind, then resolved
    /// from the file extension.
    #[napi(js_name = "sourceType")]
    pub source_type: Option<String>,
    /// Generator-stage options.
    ///
    /// By default, it is `None`, which applies the `GeneratorOptions` defaults.
    pub generator: Option<JsGeneratorOptions>,
    /// The output code options.
    ///
    /// By default, it is `None`.
    pub output: Option<JsOutputOptions>,
    /// The source-map inputs and outputs requested for the step.
    ///
    /// By default, it is `None`, which disables source-map input lookup and
    /// output delivery.
    #[napi(js_name = "sourceMap")]
    pub source_map: Option<serde_json::Value>,
    /// Raw objects for JS plugins, rooted as references.
    /// `JsPlugin::from_object` bridges them.
    ///
    /// By default, it is `[]`.
    pub plugins: Vec<ObjectRef<false>>,
}

/// JS-facing options for `generate`. The module kind is absent because the
/// generated program's own `sourceType` settles it.
#[napi_derive::napi(object)]
pub struct JsGenerateOptions {
    /// Current working directory.
    ///
    /// By default, it is the process working directory.
    pub cwd: Option<String>,
    /// The file being generated.
    ///
    /// By default, it is `"index.ts"`.
    pub file: Option<String>,
    /// The grammar of the generated code.
    ///
    /// By default, it is taken from the AST grammar, then inferred from the
    /// file extension.
    pub language: Option<String>,
    /// Generator-stage options.
    ///
    /// By default, it is `None`, which applies the `GeneratorOptions` defaults.
    pub generator: Option<JsGeneratorOptions>,
    /// The output code options.
    ///
    /// By default, it is `None`.
    pub output: Option<JsOutputOptions>,
    /// The source-map inputs and outputs requested for the step.
    ///
    /// By default, it is `None`, which disables source-map input lookup and
    /// output delivery.
    #[napi(js_name = "sourceMap")]
    pub source_map: Option<serde_json::Value>,
    /// Raw objects for JS plugins, rooted as references.
    /// `JsPlugin::from_object` bridges them.
    ///
    /// By default, it is `[]`.
    pub plugins: Vec<ObjectRef<false>>,
}

/// JS-facing parser-stage options.
#[derive(Default)]
#[napi_derive::napi(object)]
pub struct JsParserOptions {
    /// Whether parenthesized expressions stay in the AST as
    /// `ParenthesizedExpression` nodes.
    ///
    /// By default, it is `false`.
    #[napi(js_name = "preserveParens")]
    pub preserve_parens: Option<bool>,
    /// Whether `return` is accepted outside a function body.
    ///
    /// By default, it is `false`.
    #[napi(js_name = "allowReturnOutsideFunction")]
    pub allow_return_outside_function: Option<bool>,
    /// A partial patch of raw oxc parser options.
    ///
    /// Only set fields override the Telarel defaults. A curated field wins
    /// over the matching raw field. An empty patch changes nothing.
    pub oxc: Option<serde_json::Value>,
}

/// JS-facing generator-stage options.
#[derive(Default)]
#[napi_derive::napi(object)]
pub struct JsGeneratorOptions {
    /// Whether string literals use single quotes instead of double quotes.
    ///
    /// By default, it is `false`.
    #[napi(js_name = "singleQuote")]
    pub single_quote: Option<bool>,
    /// Whether non-ASCII characters are escaped in generated output.
    ///
    /// By default, it is `false`.
    #[napi(js_name = "asciiOnly")]
    pub ascii_only: Option<bool>,
    /// The indentation style for generated code.
    ///
    /// By default, it is `"space"`.
    #[napi(js_name = "indentStyle")]
    pub indent_style: Option<String>,
    /// The number of characters per indentation level.
    ///
    /// By default, it is `2`.
    #[napi(js_name = "indentWidth")]
    pub indent_width: Option<u32>,
    /// Whether comments are printed in generated code.
    ///
    /// By default, it is `true`. When effective `minify` is `true`, comments
    /// are disabled only when neither this field nor a raw `oxc.comments`
    /// field is set.
    pub comments: Option<bool>,
    /// Whether generated code is minified.
    ///
    /// By default, it is `false`.
    pub minify: Option<bool>,
    /// A partial patch of raw oxc codegen options.
    ///
    /// Only set fields override the Telarel defaults. A curated field wins
    /// over the matching raw field. An empty patch changes nothing.
    /// `sourceMapPath` is owned by Telarel and is not accepted here.
    pub oxc: Option<serde_json::Value>,
}

/// JS-facing resolved parser options for `builtinParse` (the builtin
/// `createParser`). There are no plugins, because the builtin parser never
/// runs the plugin chain.
#[napi_derive::napi(object)]
pub struct JsResolvedParserOptions {
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
    /// Whether parenthesized expressions stay in the AST.
    #[napi(js_name = "preserveParens")]
    pub preserve_parens: Option<bool>,
    /// Whether `return` is accepted outside a function body.
    #[napi(js_name = "allowReturnOutsideFunction")]
    pub allow_return_outside_function: Option<bool>,
    /// The full effective options for the raw oxc parser.
    pub oxc: Option<serde_json::Value>,
}

/// JS-facing resolved generator options for `builtinGenerate` (the builtin
/// `createGenerator`). There are no plugins, no grammar, and no module kind.
#[napi_derive::napi(object)]
pub struct JsResolvedGeneratorOptions {
    /// Current working directory. When the JS side omits it, the value
    /// resolves to the process working directory. It anchors the source file
    /// used to resolve a relative `sourceMap.input.path`.
    pub cwd: Option<String>,
    /// The file being generated. The builtin generator records it as
    /// `map.file`. `builtinGenerate` has no separate output name. When the JS
    /// side omits it, the value resolves to a default filename.
    pub file: Option<String>,
    /// The source-map inputs and outputs requested for the step.
    /// `builtinGenerate` resolves `input` and applies `output` delivery.
    #[napi(js_name = "sourceMap")]
    pub source_map: Option<serde_json::Value>,
    /// Whether string literals use single quotes.
    #[napi(js_name = "singleQuote")]
    pub single_quote: Option<bool>,
    /// Whether non-ASCII characters are escaped.
    #[napi(js_name = "asciiOnly")]
    pub ascii_only: Option<bool>,
    /// The resolved indentation style.
    #[napi(js_name = "indentStyle")]
    pub indent_style: Option<String>,
    /// The resolved indentation width.
    #[napi(js_name = "indentWidth")]
    pub indent_width: Option<u32>,
    /// Whether any comment category is printed.
    pub comments: Option<bool>,
    /// Whether generated code is minified.
    pub minify: Option<bool>,
    /// The full effective options for the raw oxc codegen.
    pub oxc: Option<serde_json::Value>,
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

pub fn parse_language_option(
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

/// The camelCase JS wire shape for raw oxc parser options.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
struct BindingOxcParseOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    parse_regular_expression: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    allow_return_outside_function: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    preserve_parens: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    allow_v8_intrinsics: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    enable_ident_hashes: Option<bool>,
}

impl BindingOxcParseOptions {
    fn into_partial(self) -> PartialOxcParseOptions {
        PartialOxcParseOptions {
            parse_regular_expression: self.parse_regular_expression,
            allow_return_outside_function: self.allow_return_outside_function,
            preserve_parens: self.preserve_parens,
            allow_v8_intrinsics: self.allow_v8_intrinsics,
            enable_ident_hashes: self.enable_ident_hashes,
        }
    }
}

impl From<&OxcParseOptions> for BindingOxcParseOptions {
    fn from(options: &OxcParseOptions) -> Self {
        Self {
            parse_regular_expression: Some(options.parse_regular_expression),
            allow_return_outside_function: Some(
                options.allow_return_outside_function,
            ),
            preserve_parens: Some(options.preserve_parens),
            allow_v8_intrinsics: Some(options.allow_v8_intrinsics),
            enable_ident_hashes: Some(options.enable_ident_hashes),
        }
    }
}

impl From<&PartialOxcParseOptions> for BindingOxcParseOptions {
    fn from(options: &PartialOxcParseOptions) -> Self {
        Self {
            parse_regular_expression: options.parse_regular_expression,
            allow_return_outside_function: options
                .allow_return_outside_function,
            preserve_parens: options.preserve_parens,
            allow_v8_intrinsics: options.allow_v8_intrinsics,
            enable_ident_hashes: options.enable_ident_hashes,
        }
    }
}

/// The JS label shared by the curated `indentStyle` and raw `indentChar`.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
enum BindingIndentStyle {
    Space,
    Tab,
}

impl From<BindingIndentStyle> for IndentChar {
    fn from(value: BindingIndentStyle) -> Self {
        match value {
            | BindingIndentStyle::Space => IndentChar::Space,
            | BindingIndentStyle::Tab => IndentChar::Tab,
        }
    }
}

impl From<IndentChar> for BindingIndentStyle {
    fn from(value: IndentChar) -> Self {
        match value {
            | IndentChar::Space => BindingIndentStyle::Space,
            | IndentChar::Tab => BindingIndentStyle::Tab,
        }
    }
}

impl From<BindingIndentStyle> for IndentStyle {
    fn from(value: BindingIndentStyle) -> Self {
        match value {
            | BindingIndentStyle::Space => IndentStyle::Space,
            | BindingIndentStyle::Tab => IndentStyle::Tab,
        }
    }
}

impl From<IndentStyle> for BindingIndentStyle {
    fn from(value: IndentStyle) -> Self {
        match value {
            | IndentStyle::Space => BindingIndentStyle::Space,
            | IndentStyle::Tab => BindingIndentStyle::Tab,
        }
    }
}

/// The JS wire shape for an oxc legal-comment policy.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
enum BindingOxcLegalComment {
    None,
    Inline,
    Eof,
    Linked(String),
    External,
}

impl From<BindingOxcLegalComment> for LegalComment {
    fn from(value: BindingOxcLegalComment) -> Self {
        match value {
            | BindingOxcLegalComment::None => LegalComment::None,
            | BindingOxcLegalComment::Inline => LegalComment::Inline,
            | BindingOxcLegalComment::Eof => LegalComment::Eof,
            | BindingOxcLegalComment::Linked(value) => {
                LegalComment::Linked(value)
            },
            | BindingOxcLegalComment::External => LegalComment::External,
        }
    }
}

impl From<&LegalComment> for BindingOxcLegalComment {
    fn from(value: &LegalComment) -> Self {
        match value {
            | LegalComment::None => BindingOxcLegalComment::None,
            | LegalComment::Inline => BindingOxcLegalComment::Inline,
            | LegalComment::Eof => BindingOxcLegalComment::Eof,
            | LegalComment::Linked(value) => {
                BindingOxcLegalComment::Linked(value.clone())
            },
            | LegalComment::External => BindingOxcLegalComment::External,
        }
    }
}

/// The camelCase JS wire shape for raw oxc comment options.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
struct BindingOxcCommentOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    normal: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    jsdoc: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    annotation: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    legal: Option<BindingOxcLegalComment>,
}

impl BindingOxcCommentOptions {
    fn into_partial(self) -> PartialOxcCommentOptions {
        PartialOxcCommentOptions {
            normal: self.normal,
            jsdoc: self.jsdoc,
            annotation: self.annotation,
            legal: self.legal.map(Into::into),
        }
    }
}

impl From<&CommentOptions> for BindingOxcCommentOptions {
    fn from(options: &CommentOptions) -> Self {
        Self {
            normal: Some(options.normal),
            jsdoc: Some(options.jsdoc),
            annotation: Some(options.annotation),
            legal: Some(BindingOxcLegalComment::from(&options.legal)),
        }
    }
}

impl From<&PartialOxcCommentOptions> for BindingOxcCommentOptions {
    fn from(options: &PartialOxcCommentOptions) -> Self {
        Self {
            normal: options.normal,
            jsdoc: options.jsdoc,
            annotation: options.annotation,
            legal: options.legal.as_ref().map(BindingOxcLegalComment::from),
        }
    }
}

/// The camelCase JS wire shape for raw oxc codegen options.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
struct BindingOxcCodegenOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    single_quote: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    minify: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ascii_only: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    comments: Option<BindingOxcCommentOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    indent_char: Option<BindingIndentStyle>,
    #[serde(skip_serializing_if = "Option::is_none")]
    indent_width: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    initial_indent: Option<u32>,
}

impl BindingOxcCodegenOptions {
    fn into_partial(self) -> PartialOxcCodegenOptions {
        PartialOxcCodegenOptions {
            single_quote: self.single_quote,
            minify: self.minify,
            ascii_only: self.ascii_only,
            comments: self.comments.map(BindingOxcCommentOptions::into_partial),
            indent_char: self.indent_char.map(Into::into),
            indent_width: self.indent_width,
            initial_indent: self.initial_indent,
        }
    }
}

impl From<&OxcCodegenOptions> for BindingOxcCodegenOptions {
    fn from(options: &OxcCodegenOptions) -> Self {
        Self {
            single_quote: Some(options.single_quote),
            minify: Some(options.minify),
            ascii_only: Some(options.ascii_only),
            comments: Some(BindingOxcCommentOptions::from(&options.comments)),
            indent_char: Some(options.indent_char.into()),
            indent_width: Some(options.indent_width),
            initial_indent: Some(options.initial_indent),
        }
    }
}

impl From<&PartialOxcCodegenOptions> for BindingOxcCodegenOptions {
    fn from(options: &PartialOxcCodegenOptions) -> Self {
        Self {
            single_quote: options.single_quote,
            minify: options.minify,
            ascii_only: options.ascii_only,
            comments: options
                .comments
                .as_ref()
                .map(BindingOxcCommentOptions::from),
            indent_char: options.indent_char.map(Into::into),
            indent_width: options.indent_width,
            initial_indent: options.initial_indent,
        }
    }
}

/// The camelCase JS wire shape for parser-stage options.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
struct BindingParserOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    preserve_parens: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    allow_return_outside_function: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    oxc: Option<serde_json::Value>,
}

impl BindingParserOptions {
    fn into_options(self) -> Result<ParserOptions> {
        Ok(ParserOptions {
            preserve_parens: self.preserve_parens,
            allow_return_outside_function: self.allow_return_outside_function,
            oxc: match self.oxc.as_ref() {
                | Some(value) => Some(parse_oxc_parser_options(value)?),
                | None => None,
            },
        })
    }
}

impl From<&ParserOptions> for BindingParserOptions {
    fn from(options: &ParserOptions) -> Self {
        Self {
            preserve_parens: options.preserve_parens,
            allow_return_outside_function: options
                .allow_return_outside_function,
            oxc: options.oxc.as_ref().map(partial_parser_oxc_to_js),
        }
    }
}

/// The camelCase JS wire shape for generator-stage options.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
struct BindingGeneratorOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    single_quote: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ascii_only: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    indent_style: Option<BindingIndentStyle>,
    #[serde(skip_serializing_if = "Option::is_none")]
    indent_width: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    comments: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    minify: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    oxc: Option<serde_json::Value>,
}

impl BindingGeneratorOptions {
    fn into_options(self) -> Result<GeneratorOptions> {
        Ok(GeneratorOptions {
            single_quote: self.single_quote,
            ascii_only: self.ascii_only,
            indent_style: self.indent_style.map(IndentStyle::from),
            indent_width: self.indent_width,
            comments: self.comments,
            minify: self.minify,
            oxc: match self.oxc.as_ref() {
                | Some(value) => Some(parse_oxc_codegen_options(value)?),
                | None => None,
            },
        })
    }
}

impl From<&GeneratorOptions> for BindingGeneratorOptions {
    fn from(options: &GeneratorOptions) -> Self {
        Self {
            single_quote: options.single_quote,
            ascii_only: options.ascii_only,
            indent_style: options.indent_style.map(BindingIndentStyle::from),
            indent_width: options.indent_width,
            comments: options.comments,
            minify: options.minify,
            oxc: options.oxc.as_ref().map(partial_codegen_oxc_to_js),
        }
    }
}

fn parse_oxc_parser_options(
    value: &serde_json::Value
) -> Result<PartialOxcParseOptions> {
    serde_json::from_value::<BindingOxcParseOptions>(value.clone())
        .map(BindingOxcParseOptions::into_partial)
        .map_err(|error| invalid_arg(format!("invalid parser.oxc: {error}")))
}

fn parse_oxc_codegen_options(
    value: &serde_json::Value
) -> Result<PartialOxcCodegenOptions> {
    serde_json::from_value::<BindingOxcCodegenOptions>(value.clone())
        .map(BindingOxcCodegenOptions::into_partial)
        .map_err(|error| invalid_arg(format!("invalid generator.oxc: {error}")))
}

/// Parse a JS parser-stage options object.
pub(crate) fn parse_parser_options(
    value: &serde_json::Value
) -> Result<ParserOptions> {
    serde_json::from_value::<BindingParserOptions>(value.clone())
        .map_err(|error| {
            invalid_arg(format!("invalid parser options: {error}"))
        })?
        .into_options()
}

/// Parse a JS generator-stage options object.
pub(crate) fn parse_generator_options(
    value: &serde_json::Value
) -> Result<GeneratorOptions> {
    serde_json::from_value::<BindingGeneratorOptions>(value.clone())
        .map_err(|error| {
            invalid_arg(format!("invalid generator options: {error}"))
        })?
        .into_options()
}

/// Serialize parser-stage options into their JS wire shape.
pub(crate) fn parser_options_to_js(
    options: &ParserOptions
) -> serde_json::Value {
    serde_json::to_value(BindingParserOptions::from(options))
        .expect("parser options serialize")
}

/// Serialize generator-stage options into their JS wire shape.
pub(crate) fn generator_options_to_js(
    options: &GeneratorOptions
) -> serde_json::Value {
    serde_json::to_value(BindingGeneratorOptions::from(options))
        .expect("generator options serialize")
}

/// Serialize the effective parser options into their JS wire shape.
pub(crate) fn parser_oxc_to_js(options: &OxcParseOptions) -> serde_json::Value {
    serde_json::to_value(BindingOxcParseOptions::from(options))
        .expect("parser oxc options serialize")
}

/// Serialize the requested parser oxc patch into its JS wire shape. Only
/// explicitly set fields are emitted.
pub(crate) fn partial_parser_oxc_to_js(
    options: &PartialOxcParseOptions
) -> serde_json::Value {
    serde_json::to_value(BindingOxcParseOptions::from(options))
        .expect("partial parser oxc options serialize")
}

/// Serialize the effective generator options into their JS wire shape.
pub(crate) fn codegen_oxc_to_js(
    options: &OxcCodegenOptions
) -> serde_json::Value {
    serde_json::to_value(BindingOxcCodegenOptions::from(options))
        .expect("codegen oxc options serialize")
}

/// Serialize the requested generator oxc patch into its JS wire shape. Only
/// explicitly set fields are emitted.
pub(crate) fn partial_codegen_oxc_to_js(
    options: &PartialOxcCodegenOptions
) -> serde_json::Value {
    serde_json::to_value(BindingOxcCodegenOptions::from(options))
        .expect("partial codegen oxc options serialize")
}

fn parse_indent_style(text: &str) -> Result<IndentStyle> {
    match text {
        | "space" => Ok(IndentStyle::Space),
        | "tab" => Ok(IndentStyle::Tab),
        | _ => Err(invalid_arg(format!(
            "invalid indentStyle {text:?}: expected \"space\" or \"tab\""
        ))),
    }
}

/// Build parser-stage options from the fields shared by the optional group and
/// the resolved builtin input.
fn parser_options_from_parts(
    preserve_parens: Option<bool>,
    allow_return_outside_function: Option<bool>,
    oxc: Option<&serde_json::Value>,
) -> Result<ParserOptions> {
    Ok(ParserOptions {
        preserve_parens,
        allow_return_outside_function,
        oxc: match oxc {
            | Some(value) => Some(parse_oxc_parser_options(value)?),
            | None => None,
        },
    })
}

/// Build generator-stage options from the fields shared by the optional group
/// and the resolved builtin input.
fn generator_options_from_parts(
    single_quote: Option<bool>,
    ascii_only: Option<bool>,
    indent_style: Option<&str>,
    indent_width: Option<u32>,
    comments: Option<bool>,
    minify: Option<bool>,
    oxc: Option<&serde_json::Value>,
) -> Result<GeneratorOptions> {
    Ok(GeneratorOptions {
        single_quote,
        ascii_only,
        indent_style: match indent_style {
            | Some(text) => Some(parse_indent_style(text)?),
            | None => None,
        },
        indent_width: indent_width.map(|width| width as usize),
        comments,
        minify,
        oxc: match oxc {
            | Some(value) => Some(parse_oxc_codegen_options(value)?),
            | None => None,
        },
    })
}

impl JsParserOptions {
    fn to_options(&self) -> Result<ParserOptions> {
        parser_options_from_parts(
            self.preserve_parens,
            self.allow_return_outside_function,
            self.oxc.as_ref(),
        )
    }
}

impl JsGeneratorOptions {
    fn to_options(&self) -> Result<GeneratorOptions> {
        generator_options_from_parts(
            self.single_quote,
            self.ascii_only,
            self.indent_style.as_deref(),
            self.indent_width,
            self.comments,
            self.minify,
            self.oxc.as_ref(),
        )
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

        let parser: Option<ParserOptions> = match &self.parser {
            | Some(parser) => Some(parser.to_options()?),
            | None => None,
        };

        Ok(ParseOptions {
            cwd: shared.cwd,
            file: shared.file,
            language: shared.language,
            source_type: shared.source_type,
            parser,
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

        let parser: Option<ParserOptions> = match &self.parser {
            | Some(parser) => Some(parser.to_options()?),
            | None => None,
        };

        let generator: Option<GeneratorOptions> = match &self.generator {
            | Some(generator) => Some(generator.to_options()?),
            | None => None,
        };

        Ok(CompileOptions {
            cwd: shared.cwd,
            file: shared.file,
            language: shared.language,
            source_type: shared.source_type,
            parser,
            generator,
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

        let generator: Option<GeneratorOptions> = match &self.generator {
            | Some(generator) => Some(generator.to_options()?),
            | None => None,
        };

        Ok(CompileFromAstOptions {
            cwd: shared.cwd,
            file: shared.file,
            language: shared.language,
            source_type: shared.source_type,
            generator,
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

        let generator: Option<GeneratorOptions> = match &self.generator {
            | Some(generator) => Some(generator.to_options()?),
            | None => None,
        };

        Ok(GenerateOptions {
            cwd: self.cwd.clone().map(PathBuf::from),
            file: self.file.clone(),
            language,
            generator,
            output: parse_output_options(self.output.as_ref())?,
            source_map,
        })
    }
}

impl JsResolvedParserOptions {
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

        let parser: ParserOptions = parser_options_from_parts(
            self.preserve_parens,
            self.allow_return_outside_function,
            self.oxc.as_ref(),
        )?;

        Ok(ResolvableOptions {
            cwd: shared.cwd,
            file: shared.file,
            language: shared.language,
            source_type: shared.source_type,
            parser: Some(parser),
            generator: None,
            output: None,
            source_map: shared.source_map,
        })
    }
}

impl JsResolvedGeneratorOptions {
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

        let generator: GeneratorOptions = generator_options_from_parts(
            self.single_quote,
            self.ascii_only,
            self.indent_style.as_deref(),
            self.indent_width,
            self.comments,
            self.minify,
            self.oxc.as_ref(),
        )?;

        Ok(ResolvableOptions {
            cwd: self.cwd.clone().map(PathBuf::from),
            file: self.file.clone(),
            language: None,
            source_type: None,
            parser: None,
            generator: Some(generator),
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

                assert_eq!(
                    json.sources,
                    vec![Some(String::from("upstream.ts"))],
                );
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

                assert_eq!(
                    json.sources,
                    vec![Some(String::from("upstream.ts"))],
                );

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

        let parser: ParserOptions =
            parsed.parser.expect("parser group present");

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

        let parsed: CompileFromAstOptions = options
            .to_compile_from_ast_options()
            .expect("generator group parses");

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

        let oxc: PartialOxcCodegenOptions =
            parsed.oxc.expect("raw oxc present");

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
            Some(oxc::codegen::LegalComment::Linked(String::from(
                "preserve-me"
            )))
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
}
