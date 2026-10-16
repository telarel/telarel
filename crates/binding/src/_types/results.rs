use telarel_common::CodeResult;

use crate::_types::sourcemap::JsSourceMap;

/// Result of the code-returning entry points (`compile`, `generate`,
/// `compileFromAst`, `builtinGenerate`) and of a `generate` hook replacement.
#[napi_derive::napi(object)]
pub struct JsCodeResult {
    /// Generated code.
    pub code: String,
    /// Source map, when a stage produced one.
    pub map: Option<JsSourceMap>,
}

impl From<CodeResult> for JsCodeResult {
    fn from(value: CodeResult) -> Self {
        Self { code: value.code, map: value.map.map(JsSourceMap::from) }
    }
}
