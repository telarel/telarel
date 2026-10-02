use napi::bindgen_prelude::{JsValue, Object, ToNapiValue};
use napi::sys;

use crate::plugin::hooks::SharedStr;

/// JS-facing plugin context passed to every ctx-bearing hook.
pub struct JsPluginContext {
    /// Current working directory.
    pub cwd: SharedStr,
    /// The original file.
    pub file: SharedStr,
    /// The original code.
    pub code: SharedStr,
    /// The grammar of the code.
    pub language: SharedStr,
    /// The module system of the code.
    pub source_type: SharedStr,
}

impl ToNapiValue for JsPluginContext {
    unsafe fn to_napi_value(
        env: sys::napi_env,
        val: Self,
    ) -> napi::Result<sys::napi_value> {
        let env: napi::Env = napi::Env::from_raw(env);

        let mut object: Object<'static> = Object::new(&env)?;

        object.set("cwd", val.cwd)?;

        object.set("file", val.file)?;

        object.set("code", val.code)?;

        object.set("language", val.language)?;

        object.set("sourceType", val.source_type)?;

        Ok(JsValue::raw(&object))
    }
}
