/// Args for the `pre` / `post` hooks.
#[napi_derive::napi(object)]
pub struct JsStageArgs {
    /// The file being compiled.
    pub file: String,
    /// The code being compiled.
    pub code: String,
}

/// Args for the `transform` hook. `astJson` is the ESTree tree as a JSON string;
/// the JS wrapper parses it before calling the user hook.
#[napi_derive::napi(object)]
pub struct JsTransformArgs {
    /// The file being compiled.
    pub file: String,
    /// The current AST as a JSON string.
    #[napi(js_name = "astJson")]
    pub ast_json: String,
}

/// Output of the `transform` hook: the possibly-mutated ESTree tree as a
/// JSON string (absent/None = pass-through).
#[napi_derive::napi(object)]
pub struct JsTransformOutput {
    /// The replacement AST as a JSON string.
    #[napi(js_name = "astJson")]
    pub ast_json: String,
}

/// Output of the `options` hook: full replacement options.
#[napi_derive::napi(object)]
pub struct JsOptionsOutput {
    /// Replacement working directory.
    pub cwd: String,
    /// Replacement file.
    pub file: String,
    /// Replacement code.
    pub code: String,
}
