use napi::bindgen_prelude::{JsValue, Object, ToNapiValue};
use napi::sys;

use crate::plugin::hooks::SharedStr;

/// Args for the `pre` / `post` hooks.
pub struct JsStageArgs {
    /// The file being compiled.
    pub file: SharedStr,
    /// The code being compiled.
    pub code: SharedStr,
}

impl ToNapiValue for JsStageArgs {
    unsafe fn to_napi_value(
        env: sys::napi_env,
        val: Self,
    ) -> napi::Result<sys::napi_value> {
        let env: napi::Env = napi::Env::from_raw(env);

        let mut object: Object<'static> = Object::new(&env)?;

        object.set("file", val.file)?;

        object.set("code", val.code)?;

        Ok(JsValue::raw(&object))
    }
}

/// Args for the `transform` hook. `astJson` is the ESTree tree as a JSON string;
/// the JS wrapper parses it before calling the user hook.
pub struct JsTransformArgs {
    /// The file being compiled.
    pub file: SharedStr,
    /// The current AST as a JSON string.
    pub ast_json: String,
}

impl ToNapiValue for JsTransformArgs {
    unsafe fn to_napi_value(
        env: sys::napi_env,
        val: Self,
    ) -> napi::Result<sys::napi_value> {
        let env: napi::Env = napi::Env::from_raw(env);

        let mut object: Object<'static> = Object::new(&env)?;

        object.set("file", val.file)?;

        object.set("astJson", val.ast_json)?;

        Ok(JsValue::raw(&object))
    }
}

/// Output of the `transform` hook: the possibly-mutated ESTree tree as a
/// JSON string (absent/None = pass-through).
#[napi_derive::napi(object)]
pub struct JsTransformOutput {
    /// The replacement AST as a JSON string.
    #[napi(js_name = "astJson")]
    pub ast_json: String,
}

/// Output of the `options` hook: a partial update;
/// `None` fields keep their current values.
#[napi_derive::napi(object)]
pub struct JsOptionsOutput {
    /// Replacement working directory, if updated.
    pub cwd: Option<String>,
    /// Replacement file, if updated.
    pub file: Option<String>,
    /// Replacement code, if updated.
    pub code: Option<String>,
}
