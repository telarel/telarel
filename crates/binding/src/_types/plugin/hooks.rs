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

/// Output of the `transform` hook: the mutated ESTree tree as a JSON string.
/// The JS wrapper sends `null` when the tree did not change (it re-stringifies
/// the tree after the hook and compares it with the pre-call string).
#[napi_derive::napi(object)]
pub struct JsTransformOutput {
    /// The replacement AST as a JSON string.
    #[napi(js_name = "astJson")]
    pub ast_json: String,
}

/// Output of the `options` hook: the full current options after the JS hook;
/// the wrapper always sends all three fields.
#[napi_derive::napi(object)]
pub struct JsOptionsOutput {
    /// Current working directory after the hook.
    pub cwd: String,
    /// Current file after the hook.
    pub file: String,
    /// Current code after the hook.
    pub code: String,
}
