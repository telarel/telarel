/// Result of `compile`.
#[napi_derive::napi(object)]
pub struct JsCompileResult {
    /// Compiled code.
    pub code: String,
    /// Source map, when a stage produced one.
    pub map: Option<crate::_types::sourcemap::JsSourceMap>,
}
