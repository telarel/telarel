/// Result of `compile`.
#[napi_derive::napi(object)]
pub struct JsCompileResult {
    /// Compiled code.
    pub code: String,
    /// Source map.
    pub map: crate::_types::sourcemap::JsSourceMap,
}
