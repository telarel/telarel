use napi::bindgen_prelude::{JsValue, Object, ToNapiValue};
use napi::sys;

use telarel_common::{
    GeneratorOptions, InputSourceMap, InputSourceMapMode, OutputOptions,
    OutputSourceMap, OutputSourceMapMode, ParserOptions, ResolvedOptions,
    SourceMapOptions,
};

use crate::_types::ast::{JsAst, JsAstInput};
use crate::_types::options::JsOutputOptions;
use crate::_types::path::path_label;
use crate::_types::sourcemap::JsSourceMap;
use crate::plugin::options::{language_label, source_type_label};

/// Args for the `transform`, `prepare` and `finalize` hooks: the current AST
/// as an ESTree JSON string plus the source text, input map and grammar it
/// anchors to. The JS wrapper reconstructs the normalized AST before calling
/// the user hook.
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

/// Output of the `transform` hook: the replacement AST. The JS wrapper sends
/// `null` when the hook returns `null`/`void` (no change); the baseline
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
    /// The grammar of the returned AST; absent means unset.
    pub language: Option<String>,
}

/// Output of the `options` hook: the fields of the options bag after the JS
/// hook. Omitted fields stay `None` ("leave unspecified"); resolution defaults
/// apply later. The plugins array is the returned bag's plugin list, as raw
/// plugin descriptor objects.
#[derive(Default)]
#[napi_derive::napi(object)]
pub struct JsOptionsOutput {
    /// Working directory used to resolve relative paths.
    pub cwd: Option<String>,
    /// The file being parsed/generated.
    pub file: Option<String>,
    /// Source language of the file.
    pub language: Option<String>,
    /// Module system used for code generation.
    #[napi(js_name = "sourceType")]
    pub source_type: Option<String>,
    /// The output code settings after the hook.
    pub output: Option<JsOutputOptions>,
    /// The source-map inputs and outputs after the hook; unspecified (`None`)
    /// is omitted from the wire.
    #[napi(js_name = "sourceMap")]
    pub source_map: Option<serde_json::Value>,
    /// The bag's plugins after the hook, as raw descriptor objects; missing
    /// means the default (empty) list.
    pub plugins: Option<Vec<napi::bindgen_prelude::ObjectRef<false>>>,
}

/// JS-facing resolved output options, as presented to the `start` hook.
#[napi_derive::napi(object)]
pub struct JsResolvedOutputOptions {
    /// The file the generated code is intended for.
    pub file: String,
}

/// JS-facing resolved options, as presented to the `start` hook.
#[napi_derive::napi(object)]
pub struct JsResolvedOptions {
    /// Working directory used to resolve relative paths.
    pub cwd: String,
    /// The file being parsed/generated.
    pub file: String,
    /// Source language of the file.
    pub language: String,
    /// Module system used for code generation.
    #[napi(js_name = "sourceType")]
    pub source_type: String,
    /// The output code settings.
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
    /// Options resolved for the hook/stage.
    pub options: JsResolvedOptions,
}

/// The JS-facing label for an output source map mode.
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

/// Convert the source-map inputs/outputs into the JS `SourceMapOptions` union.
fn source_map_options_to_js(options: &SourceMapOptions) -> serde_json::Value {
    serde_json::json!({
        "input": options.input.as_ref().map(input_source_map_to_js),
        "output": options.output.as_ref().map(output_source_map_to_js),
    })
}

impl From<&ResolvedOptions> for JsResolvedOptions {
    fn from(options: &ResolvedOptions) -> Self {
        Self {
            cwd: path_label(&options.cwd),
            file: options.file.clone(),
            language: language_label(Some(options.language)).to_owned(),
            source_type: source_type_label(Some(options.source_type))
                .to_owned(),
            output: JsResolvedOutputOptions {
                file: options.output.file.clone(),
            },
            source_map: source_map_options_to_js(&options.source_map),
            plugins: options.plugins.clone(),
        }
    }
}

/// Convert output settings into the pre-resolution JS bag shape.
///
/// An unspecified request (`None`) stays `None` and is omitted from the wire;
/// a present request (even `{}`) is transmitted.
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
/// An unspecified request (`None`) stays `None` and is omitted from the wire;
/// a present request (even an empty `{}`) is transmitted.
pub fn optional_source_map_options_to_js(
    options: Option<&SourceMapOptions>
) -> Option<serde_json::Value> {
    options.map(source_map_options_to_js)
}

/// JS-facing parser options, as presented to the `parse` hook.
#[napi_derive::napi(object)]
pub struct JsParserHookOptions {
    /// Working directory used to resolve relative paths.
    pub cwd: String,
    /// The file being parsed/generated.
    pub file: String,
    /// Source language of the file.
    pub language: String,
    /// Module system used for code generation.
    #[napi(js_name = "sourceType")]
    pub source_type: String,
    /// The source-map inputs and outputs requested for the step.
    #[napi(js_name = "sourceMap")]
    pub source_map: serde_json::Value,
}

impl From<&ParserOptions> for JsParserHookOptions {
    fn from(options: &ParserOptions) -> Self {
        Self {
            cwd: path_label(&options.cwd),
            file: options.file.clone(),
            language: language_label(Some(options.language)).to_owned(),
            source_type: source_type_label(Some(options.source_type))
                .to_owned(),
            source_map: source_map_options_to_js(&options.source_map),
        }
    }
}

/// JS-facing generator options, as presented to the `generate` hook.
#[napi_derive::napi(object)]
pub struct JsGeneratorHookOptions {
    /// Current working directory (reserved).
    pub cwd: String,
    /// The file being parsed/generated.
    pub file: String,
    /// The source-map inputs and outputs requested for the step.
    #[napi(js_name = "sourceMap")]
    pub source_map: serde_json::Value,
}

impl From<&GeneratorOptions> for JsGeneratorHookOptions {
    fn from(options: &GeneratorOptions) -> Self {
        Self {
            cwd: path_label(&options.cwd),
            file: options.file.clone(),
            source_map: source_map_options_to_js(&options.source_map),
        }
    }
}

/// Args for the `parse` hook: the code to parse and the resolved parser
/// options.
#[napi_derive::napi(object)]
pub struct JsParseArgs {
    /// Source code text.
    pub code: String,
    /// Options resolved for the hook/stage.
    pub options: JsParserHookOptions,
}

/// Return of the `parse` hook: the replacement AST, `null`/`void` (defer).
#[napi_derive::napi(object)]
pub struct JsParseOutput {
    /// The replacement AST input; an absent language is unset and ignored
    /// for grammar, which parser options own.
    pub ast: JsAstInput,
}

/// Args for the `generate` hook: the AST to generate from and the resolved
/// generator options.
#[napi_derive::napi(object)]
pub struct JsGenerateArgs {
    /// Parsed or rewritten AST.
    pub ast: JsAst,
    /// Options resolved for the hook/stage.
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
    /// Error raised by an earlier hook.
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use telarel_common::{
        InputSourceMap, InputSourceMapMode, InputSourceMapOptions, Language,
        OutputSourceMap, OutputSourceMapMode, OutputSourceMapOptions,
        SourceMapOptions, SourceType,
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
        let options: ParserOptions = ParserOptions {
            cwd: PathBuf::from("/repo"),
            file: String::from("a.tsx"),
            language: Language::TSX,
            source_type: SourceType::Module,
            source_map: SourceMapOptions {
                input: Some(InputSourceMap::Mode(InputSourceMapMode::Auto)),
                output: Some(OutputSourceMap::Mode(OutputSourceMapMode::Both)),
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
    }

    #[test]
    fn test_generator_options_to_js() {
        let options: GeneratorOptions = GeneratorOptions {
            cwd: PathBuf::from("/repo"),
            file: String::from("out.js"),
            source_map: SourceMapOptions {
                input: None,
                output: Some(OutputSourceMap::Mode(
                    OutputSourceMapMode::Inline,
                )),
            },
        };

        let js: JsGeneratorHookOptions = JsGeneratorHookOptions::from(&options);

        assert_eq!(js.cwd, "/repo");
        assert_eq!(js.file, "out.js");
        assert_eq!(
            js.source_map,
            serde_json::json!({ "input": null, "output": "inline" })
        );
    }

    #[test]
    fn test_resolved_options_to_js_carries_source_map() {
        let options: ResolvedOptions = ResolvedOptions {
            cwd: PathBuf::from("/repo"),
            file: String::from("index.ts"),
            language: Language::TS,
            source_type: SourceType::Module,
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
        assert_eq!(js.plugins, vec![String::from("observe")]);
    }

    #[test]
    fn test_resolved_options_to_js_default_source_map_is_off() {
        let options: ResolvedOptions = ResolvedOptions {
            cwd: PathBuf::from("/repo"),
            file: String::from("index.ts"),
            language: Language::TS,
            source_type: SourceType::Module,
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
    }

    #[test]
    fn test_file_passes_through_every_option_presentation() {
        let file: String = String::from("entry.ts?v=1#h");

        let resolved: ResolvedOptions = ResolvedOptions {
            cwd: PathBuf::from("/repo"),
            file: file.clone(),
            language: Language::TS,
            source_type: SourceType::Module,
            output: telarel_common::ResolvedOutputOptions {
                file: String::from("dist/index.js"),
            },
            source_map: SourceMapOptions::default(),
            plugins: Vec::new(),
        };

        let js_resolved: JsResolvedOptions = JsResolvedOptions::from(&resolved);
        let js_parser: JsParserHookOptions =
            JsParserHookOptions::from(&ParserOptions::from(&resolved));
        let js_generator: JsGeneratorHookOptions =
            JsGeneratorHookOptions::from(&GeneratorOptions::from(&resolved));

        assert_eq!(js_resolved.file, file);
        assert_eq!(js_parser.file, file);
        assert_eq!(js_generator.file, file);
    }
}
