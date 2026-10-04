use napi::bindgen_prelude::{JsValue, Object, ToNapiValue};
use napi::sys;

use crate::plugin::hooks::SharedStr;

/// Args for the `prepare` / `finalize` hooks.
pub struct JsStageArgs {
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

        object.set("code", val.code)?;

        Ok(JsValue::raw(&object))
    }
}

/// Args for the `transform` hook. `astJson` is the ESTree tree as a JSON string;
/// the JS wrapper parses it before calling the user hook.
pub struct JsTransformArgs {
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

        object.set("astJson", val.ast_json)?;

        Ok(JsValue::raw(&object))
    }
}

/// Output of the `transform` hook: the mutated ESTree tree as a JSON string.
/// The JS wrapper sends `null` when the hook returns `null`/`void` (no
/// change); the baseline string-equality skip is defense in depth for raw
/// plugin objects.
#[napi_derive::napi(object)]
pub struct JsTransformOutput {
    /// The replacement AST as a JSON string.
    #[napi(js_name = "astJson")]
    pub ast_json: String,
}

/// Output of the `options` hook: the fields of the options bag after the JS
/// hook. A full-bag replace: fields the JS hook leaves `undefined` fall back
/// to their defaults; the plugins array is the returned bag's plugin list,
/// as raw plugin descriptor objects.
#[derive(Default)]
#[napi_derive::napi(object)]
pub struct JsOptionsOutput {
    /// Current working directory after the hook.
    pub cwd: Option<String>,
    /// Current file after the hook.
    pub file: Option<String>,
    /// Current code after the hook.
    pub code: Option<String>,
    /// The grammar of the code after the hook.
    pub language: Option<String>,
    /// The module system of the code after the hook.
    pub source_type: Option<String>,
    /// The bag's plugins after the hook, as raw descriptor objects; missing
    /// means the default (empty) list.
    pub plugins: Option<Vec<napi::bindgen_prelude::ObjectRef<false>>>,
}

/// Output of the `prepare` or `finalize` hook: the replacement code with an optional
/// incremental source-map JSON. `map` serialized to the standard v3 JSON
/// shape so a JS hook can parse and forward it; `None` = no map returned.
#[napi_derive::napi(object)]
pub struct JsStageOutput {
    /// The replacement code for the stage.
    pub code: String,
    /// The incremental source map JSON, when the hook returned one.
    pub map: Option<String>,
}

/// Args for the `compileStart` hook: resolved, read-only options.
pub struct JsCompileStartArgs {
    /// Current working directory, resolved.
    pub cwd: SharedStr,
    /// The file to be compiled.
    pub file: SharedStr,
    /// The code to be compiled.
    pub code: SharedStr,
    /// The grammar of the code, resolved.
    pub language: SharedStr,
    /// The module system of the code, resolved.
    pub source_type: SharedStr,
    /// The settled plugin-name list, after the options fixpoint.
    pub plugins: Vec<String>,
}

impl ToNapiValue for JsCompileStartArgs {
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

        object.set("plugins", val.plugins)?;

        Ok(JsValue::raw(&object))
    }
}

/// Args for the `compileEnd` hook: the last-good output and the error, when
/// the compile failed. `map` is the standard v3 source-map JSON, or `None`
/// when no map was produced.
pub struct JsCompileEndArgs {
    /// The compiled code, or the last good code on error.
    pub code: SharedStr,
    /// The source map JSON, or the last good map on error; `None` = no map.
    pub map: Option<String>,
    /// The error message, when the compile failed.
    pub err: Option<String>,
}

impl ToNapiValue for JsCompileEndArgs {
    unsafe fn to_napi_value(
        env: sys::napi_env,
        val: Self,
    ) -> napi::Result<sys::napi_value> {
        let env: napi::Env = napi::Env::from_raw(env);

        let mut object: Object<'static> = Object::new(&env)?;

        object.set("code", val.code)?;

        if let Some(map) = val.map {
            object.set("map", map)?;
        }

        if let Some(err) = val.err {
            object.set("err", err)?;
        }

        Ok(JsValue::raw(&object))
    }
}
