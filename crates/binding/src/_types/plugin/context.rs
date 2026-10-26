use napi::bindgen_prelude::{JsValue, Object, ToNapiValue};
use napi::sys;

use crate::plugin::hooks::SharedStr;

/// The JS-facing plugin context that every context-bearing hook receives.
pub struct JsPluginContext {
    /// Working directory used to resolve relative paths.
    pub cwd: SharedStr,
    /// The file being parsed or generated.
    pub file: SharedStr,
    /// Source language of the file.
    pub language: SharedStr,
    /// Module system used for code generation.
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

        object.set("language", val.language)?;

        object.set("sourceType", val.source_type)?;

        Ok(JsValue::raw(&object))
    }
}
