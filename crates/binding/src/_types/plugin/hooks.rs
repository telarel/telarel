use napi::bindgen_prelude::{Array, JsValue, Object, ToNapiValue};
use napi::sys;

use telarel_common::{
    IndentStyle, InputSourceMap, InputSourceMapMode, OutputOptions,
    OutputSourceMap, OutputSourceMapMode, ResolvedGeneratorOptions,
    ResolvedOptions, ResolvedParserOptions, SourceMapOptions,
};

use crate::_types::ast::{JsAst, JsAstInput};
use crate::_types::options::{
    JsOutputOptions, codegen_oxc_to_js, language_label, parser_oxc_to_js,
    source_type_label,
};
use crate::_types::path::path_label;
use crate::_types::refs::SharedRef;
use crate::_types::sourcemap::JsSourceMap;

/// Args for the `transform`, `prepare`, and `finalize` hooks. The payload
/// holds the current AST as an ESTree JSON string. It also holds the source
/// text, input map, and grammar that the AST anchors to. The JS wrapper
/// reconstructs the normalized AST before it calls the user hook.
pub struct JsAstArgs {
    /// AST serialized as JSON for the JS bridge.
    pub ast_json: String,
    /// Source text the program's spans anchor to.
    pub code: String,
    /// The input source map, when the AST carries one.
    pub map: Option<JsSourceMap>,
    /// The grammar of the AST, as its canonical label.
    pub language: String,
}

impl ToNapiValue for JsAstArgs {
    unsafe fn to_napi_value(
        env: sys::napi_env,
        val: Self,
    ) -> napi::Result<sys::napi_value> {
        let env: napi::Env = napi::Env::from_raw(env);

        let mut object: Object<'static> = Object::new(&env)?;

        object.set("astJson", val.ast_json)?;

        object.set("code", val.code)?;

        if let Some(map) = val.map {
            object.set("map", map)?;
        }

        object.set("language", val.language)?;

        Ok(JsValue::raw(&object))
    }
}

/// Args for a batched `transform` run. The payload carries the carried AST,
/// its source text, map, and grammar, plus the raw plugin descriptors for the
/// run in settled order. The JS batch runner invokes each descriptor's
/// `__TRANSFORM_ENTRY__` in order over the same JS AST.
pub struct JsTransformBatchArgs {
    /// AST serialized as JSON for the JS bridge.
    pub ast_json: String,
    /// Source text the program's spans anchor to.
    pub code: String,
    /// The input source map, when the AST carries one.
    pub map: Option<JsSourceMap>,
    /// The grammar of the AST, as its canonical label.
    pub language: String,
    /// The raw plugin descriptors for this run, in driver order.
    pub plugins: Vec<SharedRef>,
}

impl ToNapiValue for JsTransformBatchArgs {
    unsafe fn to_napi_value(
        env: sys::napi_env,
        val: Self,
    ) -> napi::Result<sys::napi_value> {
        let env: napi::Env = napi::Env::from_raw(env);

        let mut object: Object<'static> = Object::new(&env)?;

        object.set("astJson", val.ast_json)?;

        object.set("code", val.code)?;

        if let Some(map) = val.map {
            object.set("map", map)?;
        }

        object.set("language", val.language)?;

        let descriptors: Vec<Object<'static>> = val
            .plugins
            .iter()
            .map(|reference| reference.get(&env))
            .collect::<napi::Result<Vec<Object<'static>>>>()?;

        let plugins: Array<'_> = Array::from_vec(
            &env,
            descriptors.iter().collect::<Vec<&Object<'static>>>(),
        )?;

        object.set("plugins", plugins)?;

        Ok(JsValue::raw(&object))
    }
}

/// Output of the `transform` hook: the replacement AST. The JS wrapper sends
/// `null` when the hook returns `null` or `void` (no change). The baseline
/// equality skip is defense in depth for raw plugin objects.
#[napi_derive::napi(object)]
pub struct JsTransformOutput {
    /// AST serialized as JSON for the JS bridge.
    #[napi(js_name = "astJson")]
    pub ast_json: String,
    /// Source text the program's spans anchor to.
    pub code: String,
    /// The input source map, when the AST carries one.
    pub map: Option<JsSourceMap>,
    /// The grammar of the returned AST. Absent means unset.
    pub language: Option<String>,
}

/// Output of the `options` hook: an option patch returned by the JS hook.
///
/// The patch is inbound. The JS wrapper omits every field the hook left
/// unchanged, and Rust keeps the settled value for an absent field. The
/// `plugins` list carries only the descriptors the hook added, as raw
/// descriptor objects for plugins.
#[derive(Default)]
#[napi_derive::napi(object)]
pub struct JsOptionsOutput {
    /// Working directory used to resolve relative paths.
    ///
    /// By default, it is absent and the settled value is kept.
    pub cwd: Option<String>,
    /// The file being parsed or generated.
    ///
    /// By default, it is absent and the settled value is kept.
    pub file: Option<String>,
    /// Source language of the file.
    ///
    /// By default, it is absent and the settled value is kept.
    pub language: Option<String>,
    /// Module system used for code generation.
    ///
    /// By default, it is absent and the settled value is kept.
    #[napi(js_name = "sourceType")]
    pub source_type: Option<String>,
    /// Parser-stage options after the hook.
    ///
    /// By default, it is absent and the settled group is kept.
    pub parser: Option<serde_json::Value>,
    /// Generator-stage options after the hook.
    ///
    /// By default, it is absent and the settled group is kept.
    pub generator: Option<serde_json::Value>,
    /// The output code options after the hook.
    ///
    /// By default, it is absent and the settled group is kept.
    pub output: Option<JsOutputOptions>,
    /// The source-map inputs and outputs after the hook.
    ///
    /// By default, it is absent and the settled group is kept.
    #[napi(js_name = "sourceMap")]
    pub source_map: Option<serde_json::Value>,
    /// The descriptors the hook added, as raw descriptor objects.
    ///
    /// By default, it is absent, which adds no plugin. Descriptors already in
    /// the payload's `plugins` list are never repeated here.
    pub plugins: Option<Vec<napi::bindgen_prelude::ObjectRef<false>>>,
}

/// Resolved output options for the JS side, as presented to the `start` hook.
#[napi_derive::napi(object)]
pub struct JsResolvedOutputOptions {
    /// The intended file for the generated code.
    pub file: String,
}

/// Effective parser-stage settings for the JS side, as presented to `start`.
///
/// `oxc` is the full merged object and the curated fields mirror it.
#[napi_derive::napi(object)]
pub struct JsResolvedParserStageOptions {
    /// Whether parenthesized expressions stay in the AST.
    #[napi(js_name = "preserveParens")]
    pub preserve_parens: bool,
    /// Whether `return` is accepted outside a function body.
    #[napi(js_name = "allowReturnOutsideFunction")]
    pub allow_return_outside_function: bool,
    /// The effective options for the raw oxc parser.
    pub oxc: serde_json::Value,
}

/// Effective generator-stage settings for the JS side, as presented to `start`.
///
/// `oxc` is the full merged object and the curated fields mirror it.
#[napi_derive::napi(object)]
pub struct JsResolvedGeneratorStageOptions {
    /// Whether string literals use single quotes.
    #[napi(js_name = "singleQuote")]
    pub single_quote: bool,
    /// Whether non-ASCII characters are escaped.
    #[napi(js_name = "asciiOnly")]
    pub ascii_only: bool,
    /// The resolved indentation style.
    #[napi(js_name = "indentStyle")]
    pub indent_style: String,
    /// The resolved indentation width.
    #[napi(js_name = "indentWidth")]
    pub indent_width: u32,
    /// Whether any comment category is printed.
    pub comments: bool,
    /// Whether generated code is minified.
    pub minify: bool,
    /// The effective options for the raw oxc codegen.
    pub oxc: serde_json::Value,
}

/// Resolved options for the JS side, as presented to the `start` hook.
#[napi_derive::napi(object)]
pub struct JsResolvedOptions {
    /// Working directory used to resolve relative paths.
    pub cwd: String,
    /// The file being parsed or generated.
    pub file: String,
    /// Source language of the file.
    pub language: String,
    /// Module system used for code generation.
    #[napi(js_name = "sourceType")]
    pub source_type: String,
    /// Resolved parser-stage settings.
    pub parser: JsResolvedParserStageOptions,
    /// Resolved generator-stage settings.
    pub generator: JsResolvedGeneratorStageOptions,
    /// The output code options.
    pub output: JsResolvedOutputOptions,
    /// The source-map inputs and outputs requested for the run.
    #[napi(js_name = "sourceMap")]
    pub source_map: serde_json::Value,
    /// The settled plugin-name list, after the options fixpoint.
    pub plugins: Vec<String>,
}

/// Args for the `start` hook: the resolved, read-only options.
#[napi_derive::napi(object)]
pub struct JsStartArgs {
    /// Options resolved for the hook or stage.
    pub options: JsResolvedOptions,
}

/// The JS label for a mode of an output source map.
fn output_mode_label(mode: OutputSourceMapMode) -> &'static str {
    match mode {
        | OutputSourceMapMode::Inline => "inline",
        | OutputSourceMapMode::External => "external",
        | OutputSourceMapMode::Both => "both",
    }
}

/// Convert an input source-map request into its JS union value.
fn input_source_map_to_js(input: &InputSourceMap) -> serde_json::Value {
    match input {
        | InputSourceMap::Mode(mode) => {
            let label: &str = match mode {
                | InputSourceMapMode::Value(map) => {
                    return serde_json::from_str(&map.to_json_string())
                        .unwrap_or(serde_json::Value::Null);
                },
                | InputSourceMapMode::Inline => "inline",
                | InputSourceMapMode::External => "external",
                | InputSourceMapMode::Auto => "auto",
            };

            serde_json::Value::String(label.to_owned())
        },
        | InputSourceMap::Options(options) => {
            let label: &str = match &options.mode {
                | InputSourceMapMode::Value(map) => {
                    return serde_json::from_str(&map.to_json_string())
                        .unwrap_or(serde_json::Value::Null);
                },
                | InputSourceMapMode::Inline => "inline",
                | InputSourceMapMode::External => "external",
                | InputSourceMapMode::Auto => "auto",
            };

            let mut object: serde_json::Map<String, serde_json::Value> =
                serde_json::Map::new();

            object.insert(
                String::from("mode"),
                serde_json::Value::String(label.to_owned()),
            );

            if let Some(path) = options.path.as_deref() {
                object.insert(
                    String::from("path"),
                    serde_json::Value::String(path_label(path)),
                );
            }

            serde_json::Value::Object(object)
        },
    }
}

/// Convert an output source-map request into its JS union value.
fn output_source_map_to_js(output: &OutputSourceMap) -> serde_json::Value {
    match output {
        | OutputSourceMap::Mode(mode) => {
            serde_json::Value::String(output_mode_label(*mode).to_owned())
        },
        | OutputSourceMap::Options(options) => {
            let mut object: serde_json::Map<String, serde_json::Value> =
                serde_json::Map::new();

            object.insert(
                String::from("mode"),
                serde_json::Value::String(
                    output_mode_label(options.mode).to_owned(),
                ),
            );

            if let Some(url) = options.url.as_deref() {
                object.insert(
                    String::from("url"),
                    serde_json::Value::String(url.to_owned()),
                );
            }

            serde_json::Value::Object(object)
        },
    }
}

/// Convert the source-map inputs and outputs into the JS `SourceMapOptions`
/// union.
fn source_map_options_to_js(options: &SourceMapOptions) -> serde_json::Value {
    serde_json::json!({
        "input": options.input.as_ref().map(input_source_map_to_js),
        "output": options.output.as_ref().map(output_source_map_to_js),
    })
}

/// The JS label for an indentation style.
fn indent_style_label(style: IndentStyle) -> String {
    match style {
        | IndentStyle::Space => String::from("space"),
        | IndentStyle::Tab => String::from("tab"),
    }
}

impl From<&ResolvedOptions> for JsResolvedOptions {
    fn from(options: &ResolvedOptions) -> Self {
        Self {
            cwd: path_label(&options.cwd),
            file: options.file.clone(),
            language: language_label(Some(options.language)).to_owned(),
            source_type: source_type_label(Some(options.source_type))
                .to_owned(),
            parser: JsResolvedParserStageOptions {
                preserve_parens: options.parser.preserve_parens,
                allow_return_outside_function: options
                    .parser
                    .allow_return_outside_function,
                oxc: parser_oxc_to_js(&options.parser.oxc),
            },
            generator: JsResolvedGeneratorStageOptions {
                single_quote: options.generator.single_quote,
                ascii_only: options.generator.ascii_only,
                indent_style: indent_style_label(
                    options.generator.indent_style,
                ),
                indent_width: options.generator.indent_width as u32,
                comments: options.generator.comments,
                minify: options.generator.minify,
                oxc: codegen_oxc_to_js(&options.generator.oxc),
            },
            output: JsResolvedOutputOptions {
                file: options.output.file.clone(),
            },
            source_map: source_map_options_to_js(&options.source_map),
            plugins: options.plugins.clone(),
        }
    }
}

/// Convert output options into the pre-resolution JS bag shape.
///
/// An unspecified request (`None`) stays `None`, and Telarel omits it from
/// the wire. Telarel transmits a present request, even `{}`.
pub fn optional_output_options_to_js(
    options: Option<&OutputOptions>
) -> Option<serde_json::Value> {
    options.map(|options: &OutputOptions| {
        let mut object: serde_json::Map<String, serde_json::Value> =
            serde_json::Map::new();

        if let Some(file) = options.file.as_deref() {
            object.insert(
                String::from("file"),
                serde_json::Value::String(file.to_owned()),
            );
        }

        serde_json::Value::Object(object)
    })
}

/// Convert an optional source-map request into its JS union value.
///
/// An unspecified request (`None`) stays `None`, and Telarel omits it from
/// the wire. Telarel transmits a present request, even an empty `{}`.
pub fn optional_source_map_options_to_js(
    options: Option<&SourceMapOptions>
) -> Option<serde_json::Value> {
    options.map(source_map_options_to_js)
}

/// Parser options presented to the `parse` hook. `oxc` is the full effective
/// object and the curated fields mirror it.
#[napi_derive::napi(object)]
pub struct JsParserHookOptions {
    /// Working directory used to resolve relative paths.
    pub cwd: String,
    /// The file being parsed or generated.
    pub file: String,
    /// Source language of the file.
    pub language: String,
    /// Module system used for code generation.
    #[napi(js_name = "sourceType")]
    pub source_type: String,
    /// The source-map inputs and outputs requested for the step.
    #[napi(js_name = "sourceMap")]
    pub source_map: serde_json::Value,
    /// Whether parenthesized expressions stay in the AST.
    #[napi(js_name = "preserveParens")]
    pub preserve_parens: bool,
    /// Whether `return` is accepted outside a function body.
    #[napi(js_name = "allowReturnOutsideFunction")]
    pub allow_return_outside_function: bool,
    /// The effective options for the raw oxc parser.
    pub oxc: serde_json::Value,
}

impl From<&ResolvedParserOptions> for JsParserHookOptions {
    fn from(options: &ResolvedParserOptions) -> Self {
        Self {
            cwd: path_label(&options.cwd),
            file: options.file.clone(),
            language: language_label(Some(options.language)).to_owned(),
            source_type: source_type_label(Some(options.source_type))
                .to_owned(),
            source_map: source_map_options_to_js(&options.source_map),
            preserve_parens: options.preserve_parens,
            allow_return_outside_function: options
                .allow_return_outside_function,
            oxc: parser_oxc_to_js(&options.oxc),
        }
    }
}

/// Generator options presented to the `generate` hook. `oxc` is the full
/// effective object and the curated fields mirror it.
#[napi_derive::napi(object)]
pub struct JsGeneratorHookOptions {
    /// Current working directory (reserved).
    pub cwd: String,
    /// The file being parsed or generated.
    pub file: String,
    /// The source-map request for the step. A generate hook sees no input or
    /// output request because the pipeline resolved the input before the hook
    /// and owns output delivery. The wire shape stays
    /// `{ input: null, output: null }`.
    #[napi(js_name = "sourceMap")]
    pub source_map: serde_json::Value,
    /// Whether string literals use single quotes.
    #[napi(js_name = "singleQuote")]
    pub single_quote: bool,
    /// Whether non-ASCII characters are escaped.
    #[napi(js_name = "asciiOnly")]
    pub ascii_only: bool,
    /// The resolved indentation style.
    #[napi(js_name = "indentStyle")]
    pub indent_style: String,
    /// The resolved indentation width.
    #[napi(js_name = "indentWidth")]
    pub indent_width: u32,
    /// Whether any comment category is printed.
    pub comments: bool,
    /// Whether generated code is minified.
    pub minify: bool,
    /// The effective options for the raw oxc codegen.
    pub oxc: serde_json::Value,
}

impl From<&ResolvedGeneratorOptions> for JsGeneratorHookOptions {
    fn from(options: &ResolvedGeneratorOptions) -> Self {
        Self {
            cwd: path_label(&options.cwd),
            file: options.file.clone(),
            source_map: source_map_options_to_js(&options.source_map),
            single_quote: options.single_quote,
            ascii_only: options.ascii_only,
            indent_style: indent_style_label(options.indent_style),
            indent_width: options.indent_width as u32,
            comments: options.comments,
            minify: options.minify,
            oxc: codegen_oxc_to_js(&options.oxc),
        }
    }
}

/// Args for the `parse` hook: the code to parse and the resolved parser
/// options.
#[napi_derive::napi(object)]
pub struct JsParseArgs {
    /// Source code text.
    pub code: String,
    /// Options resolved for the hook or stage.
    pub options: JsParserHookOptions,
}

/// Return of the `parse` hook: the replacement AST. `null` or `void` means no
/// change.
#[napi_derive::napi(object)]
pub struct JsParseOutput {
    /// The replacement AST input. An absent language is unset and ignored for
    /// grammar, which parser options own.
    pub ast: JsAstInput,
}

/// Args for the `generate` hook: the AST to generate from and the resolved
/// generator options.
#[napi_derive::napi(object)]
pub struct JsGenerateArgs {
    /// Parsed or rewritten AST.
    pub ast: JsAst,
    /// Options resolved for the hook or stage.
    pub options: JsGeneratorHookOptions,
}

/// Return of the `generate` hook: the generated code with an optional source
/// map object.
#[napi_derive::napi(object)]
pub struct JsCodeOutput {
    /// Source code text.
    pub code: String,
    /// The incremental source map, when the hook returned one.
    pub map: Option<JsSourceMap>,
}

/// Args for the `end` hook: the rendered compile error, or `null` on success.
#[napi_derive::napi(object)]
pub struct JsEndArgs {
    /// An error that an earlier hook raised.
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use telarel_common::{
        InputSourceMap, InputSourceMapMode, InputSourceMapOptions, Language,
        OutputSourceMap, OutputSourceMapMode, OutputSourceMapOptions,
        OxcCodegenOptions, OxcParseOptions, ResolvedGeneratorStageOptions,
        ResolvedParserStageOptions, SourceMapOptions, SourceType,
    };

    use super::*;

    #[test]
    fn test_source_map_options_default_is_off() {
        let value: serde_json::Value =
            source_map_options_to_js(&SourceMapOptions::default());

        assert_eq!(value, serde_json::json!({ "input": null, "output": null }));
    }

    #[test]
    fn test_optional_output_options_to_js_omits_unspecified() {
        assert_eq!(optional_output_options_to_js(None), None);
    }

    #[test]
    fn test_optional_output_options_to_js_keeps_file() {
        let options: OutputOptions =
            OutputOptions { file: Some(String::from("dist/index.js")) };

        assert_eq!(
            optional_output_options_to_js(Some(&options)),
            Some(serde_json::json!({ "file": "dist/index.js" })),
        );
    }

    #[test]
    fn test_optional_source_map_options_to_js_omits_unspecified() {
        assert_eq!(optional_source_map_options_to_js(None), None);

        assert_eq!(
            optional_source_map_options_to_js(Some(&SourceMapOptions {
                input: Some(InputSourceMap::Mode(InputSourceMapMode::Auto)),
                output: None,
            })),
            Some(serde_json::json!({ "input": "auto", "output": null }))
        );
    }

    #[test]
    fn test_source_map_options_mode_labels() {
        let options: SourceMapOptions = SourceMapOptions {
            input: Some(InputSourceMap::Mode(InputSourceMapMode::Auto)),
            output: Some(OutputSourceMap::Mode(OutputSourceMapMode::Both)),
        };

        let value: serde_json::Value = source_map_options_to_js(&options);

        assert_eq!(
            value,
            serde_json::json!({ "input": "auto", "output": "both" })
        );
    }

    #[test]
    fn test_source_map_options_output_full_shape() {
        let options: SourceMapOptions = SourceMapOptions {
            input: Some(InputSourceMap::Mode(InputSourceMapMode::Inline)),
            output: Some(OutputSourceMap::Options(OutputSourceMapOptions {
                mode: OutputSourceMapMode::External,
                url: None,
            })),
        };

        let value: serde_json::Value = source_map_options_to_js(&options);

        assert_eq!(
            value,
            serde_json::json!({
                "input": "inline",
                "output": { "mode": "external" },
            })
        );
    }

    #[test]
    fn test_source_map_options_input_options_with_path() {
        let options: SourceMapOptions = SourceMapOptions {
            input: Some(InputSourceMap::Options(InputSourceMapOptions {
                mode: InputSourceMapMode::External,
                path: Some(PathBuf::from("maps/app.js.map")),
            })),
            output: None,
        };

        let value: serde_json::Value = source_map_options_to_js(&options);

        assert_eq!(
            value,
            serde_json::json!({
                "input": {
                    "mode": "external",
                    "path": "maps/app.js.map",
                },
                "output": null,
            })
        );
    }

    #[test]
    fn test_source_map_options_input_options_without_path() {
        let options: SourceMapOptions = SourceMapOptions {
            input: Some(InputSourceMap::Options(InputSourceMapOptions {
                mode: InputSourceMapMode::Inline,
                path: None,
            })),
            output: None,
        };

        let value: serde_json::Value = source_map_options_to_js(&options);

        assert_eq!(
            value,
            serde_json::json!({
                "input": { "mode": "inline" },
                "output": null,
            })
        );
    }

    #[test]
    fn test_source_map_options_output_options_with_url() {
        let options: SourceMapOptions = SourceMapOptions {
            input: None,
            output: Some(OutputSourceMap::Options(OutputSourceMapOptions {
                mode: OutputSourceMapMode::External,
                url: Some(String::from("dist/app.js.map")),
            })),
        };

        let value: serde_json::Value = source_map_options_to_js(&options);

        assert_eq!(
            value,
            serde_json::json!({
                "input": null,
                "output": {
                    "mode": "external",
                    "url": "dist/app.js.map",
                },
            })
        );
    }

    #[test]
    fn test_parser_options_to_js_labels() {
        let options: ResolvedParserOptions = ResolvedParserOptions {
            cwd: PathBuf::from("/repo"),
            file: String::from("a.tsx"),
            language: Language::TSX,
            source_type: SourceType::Module,
            source_map: SourceMapOptions {
                input: Some(InputSourceMap::Mode(InputSourceMapMode::Auto)),
                output: Some(OutputSourceMap::Mode(OutputSourceMapMode::Both)),
            },
            preserve_parens: true,
            allow_return_outside_function: true,
            oxc: OxcParseOptions {
                preserve_parens: true,
                allow_return_outside_function: true,
                enable_ident_hashes: false,
                ..OxcParseOptions::default()
            },
        };

        let js: JsParserHookOptions = JsParserHookOptions::from(&options);

        assert_eq!(js.cwd, "/repo");
        assert_eq!(js.file, "a.tsx");
        assert_eq!(js.language, "tsx");
        assert_eq!(js.source_type, "module");
        assert_eq!(
            js.source_map,
            serde_json::json!({ "input": "auto", "output": "both" })
        );
        assert!(js.preserve_parens);
        assert!(js.allow_return_outside_function);
        assert_eq!(
            js.oxc,
            serde_json::json!({
                "parseRegularExpression": false,
                "allowReturnOutsideFunction": true,
                "preserveParens": true,
                "allowV8Intrinsics": false,
                "enableIdentHashes": false,
            })
        );
    }

    #[test]
    fn test_generator_options_to_js() {
        let options: ResolvedGeneratorOptions = ResolvedGeneratorOptions {
            cwd: PathBuf::from("/repo"),
            file: String::from("out.js"),
            source_map: SourceMapOptions {
                input: None,
                output: Some(OutputSourceMap::Mode(
                    OutputSourceMapMode::Inline,
                )),
            },
            single_quote: true,
            ascii_only: true,
            indent_style: IndentStyle::Tab,
            indent_width: 4,
            comments: false,
            minify: true,
            oxc: OxcCodegenOptions {
                single_quote: true,
                ascii_only: true,
                indent_char: oxc::codegen::IndentChar::Tab,
                indent_width: 4,
                comments: oxc::codegen::CommentOptions::disabled(),
                minify: true,
                ..OxcCodegenOptions::default()
            },
        };

        let js: JsGeneratorHookOptions = JsGeneratorHookOptions::from(&options);

        assert_eq!(js.cwd, "/repo");
        assert_eq!(js.file, "out.js");
        assert_eq!(
            js.source_map,
            serde_json::json!({ "input": null, "output": "inline" })
        );
        assert!(js.single_quote);
        assert!(js.ascii_only);
        assert_eq!(js.indent_style, "tab");
        assert_eq!(js.indent_width, 4);
        assert!(!js.comments);
        assert!(js.minify);
        assert_eq!(js.oxc["indentChar"], "tab");
        assert_eq!(js.oxc["indentWidth"], 4);
        assert_eq!(js.oxc["minify"], true);
        assert_eq!(js.oxc["comments"]["normal"], false);
    }

    #[test]
    fn test_resolved_options_to_js_carries_source_map() {
        let options: ResolvedOptions = ResolvedOptions {
            cwd: PathBuf::from("/repo"),
            file: String::from("index.ts"),
            language: Language::TS,
            source_type: SourceType::Module,
            parser: ResolvedParserStageOptions {
                preserve_parens: true,
                allow_return_outside_function: true,
                oxc: OxcParseOptions {
                    preserve_parens: true,
                    allow_return_outside_function: true,
                    ..OxcParseOptions::default()
                },
            },
            generator: ResolvedGeneratorStageOptions {
                single_quote: true,
                ascii_only: true,
                indent_style: IndentStyle::Tab,
                indent_width: 4,
                comments: false,
                minify: true,
                oxc: OxcCodegenOptions {
                    single_quote: true,
                    ascii_only: true,
                    indent_char: oxc::codegen::IndentChar::Tab,
                    indent_width: 4,
                    comments: oxc::codegen::CommentOptions::disabled(),
                    minify: true,
                    ..OxcCodegenOptions::default()
                },
            },
            output: telarel_common::ResolvedOutputOptions {
                file: String::from("dist/index.js"),
            },
            source_map: SourceMapOptions {
                input: Some(InputSourceMap::Mode(InputSourceMapMode::Auto)),
                output: Some(OutputSourceMap::Mode(OutputSourceMapMode::Both)),
            },
            plugins: vec![String::from("observe")],
        };

        let js: JsResolvedOptions = JsResolvedOptions::from(&options);

        assert_eq!(js.cwd, "/repo");
        assert_eq!(js.file, "index.ts");
        assert_eq!(js.language, "ts");
        assert_eq!(js.source_type, "module");
        assert_eq!(js.output.file, "dist/index.js");
        assert_eq!(
            js.source_map,
            serde_json::json!({ "input": "auto", "output": "both" })
        );
        assert!(js.parser.preserve_parens);
        assert!(js.parser.allow_return_outside_function);
        assert_eq!(js.parser.oxc["preserveParens"], true);
        assert!(js.generator.single_quote);
        assert!(js.generator.ascii_only);
        assert_eq!(js.generator.indent_style, "tab");
        assert_eq!(js.generator.indent_width, 4);
        assert!(!js.generator.comments);
        assert!(js.generator.minify);
        assert_eq!(js.generator.oxc["indentChar"], "tab");
        assert_eq!(js.plugins, vec![String::from("observe")]);
    }

    #[test]
    fn test_resolved_options_to_js_default_source_map_is_off() {
        let options: ResolvedOptions = ResolvedOptions {
            cwd: PathBuf::from("/repo"),
            file: String::from("index.ts"),
            language: Language::TS,
            source_type: SourceType::Module,
            parser: ResolvedParserStageOptions::default(),
            generator: ResolvedGeneratorStageOptions::default(),
            output: telarel_common::ResolvedOutputOptions {
                file: String::from("dist/index.js"),
            },
            source_map: SourceMapOptions::default(),
            plugins: Vec::new(),
        };

        let js: JsResolvedOptions = JsResolvedOptions::from(&options);

        assert_eq!(
            js.source_map,
            serde_json::json!({ "input": null, "output": null })
        );
        assert!(!js.parser.preserve_parens);
        assert!(!js.generator.minify);
        assert_eq!(js.generator.indent_style, "space");
        assert_eq!(js.generator.indent_width, 2);
    }

    #[test]
    fn test_file_passes_through_every_option_presentation() {
        let file: String = String::from("entry.ts?v=1#h");

        let resolved: ResolvedOptions = ResolvedOptions {
            cwd: PathBuf::from("/repo"),
            file: file.clone(),
            language: Language::TS,
            source_type: SourceType::Module,
            parser: ResolvedParserStageOptions::default(),
            generator: ResolvedGeneratorStageOptions::default(),
            output: telarel_common::ResolvedOutputOptions {
                file: String::from("dist/index.js"),
            },
            source_map: SourceMapOptions::default(),
            plugins: Vec::new(),
        };

        let js_resolved: JsResolvedOptions = JsResolvedOptions::from(&resolved);
        let js_parser: JsParserHookOptions =
            JsParserHookOptions::from(&ResolvedParserOptions::from(&resolved));
        let js_generator: JsGeneratorHookOptions = JsGeneratorHookOptions::from(
            &ResolvedGeneratorOptions::from(&resolved),
        );

        assert_eq!(js_resolved.file, file);
        assert_eq!(js_parser.file, file);
        assert_eq!(js_generator.file, file);
    }
}
