use napi::bindgen_prelude::ObjectRef;

use telarel_common::CompileOptions;

/// JS-facing compile options.
#[napi_derive::napi(object)]
pub struct JsOptions {
    /// Current working directory.
    pub cwd: String,
    /// The file to compile.
    pub file: String,
    /// The source code to compile.
    pub code: String,
    /// Raw JS plugin objects, rooted as references;
    /// bridged by `JsPlugin::from_object`.
    pub plugins: Vec<ObjectRef<false>>,
}

/// Convert JS options into Rust options (plugins are bridged separately).
pub fn to_compile_options(options: &JsOptions) -> CompileOptions {
    CompileOptions {
        cwd: options.cwd.clone(),
        file: options.file.clone(),
        code: options.code.clone(),
    }
}
