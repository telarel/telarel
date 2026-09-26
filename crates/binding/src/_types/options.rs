use napi::bindgen_prelude::ObjectRef;
use napi::{Error, Result, Status};

use telarel_common::{CompileOptions, Language, SourceType};

/// JS-facing compile options.
#[napi_derive::napi(object)]
pub struct JsOptions {
    /// Current working directory;
    /// omitted on the JS side resolves to the process working directory.
    pub cwd: Option<String>,
    /// The file to compile.
    pub file: String,
    /// The source code to compile.
    pub code: String,
    /// The grammar of the code;
    /// omitted on the JS side infers the grammar from the file extension.
    pub language: Option<String>,
    /// The module system of the code;
    /// omitted on the JS side keeps the module kind resolved from the grammar.
    #[napi(js_name = "sourceType")]
    pub source_type: Option<String>,
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

/// Convert JS options into Rust options (plugins are bridged separately).
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

    Ok(CompileOptions {
        cwd: options.cwd.clone(),
        file: options.file.clone(),
        code: options.code.clone(),
        language,
        source_type,
    })
}

#[cfg(test)]
mod tests {
    use telarel_common::{Language, SourceType};

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
        assert_eq!(parsed.file, "index.js");
        assert_eq!(parsed.code, "const a = 1;");
        assert_eq!(parsed.cwd, None);
    }

    #[test]
    fn test_to_compile_options_keeps_missing_options_none() {
        let options: JsOptions = fixtures(None, None);

        let parsed: telarel_common::CompileOptions =
            to_compile_options(&options).expect("absent options parse");

        assert_eq!(parsed.language, None);
        assert_eq!(parsed.source_type, None);
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
}
