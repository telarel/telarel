use napi::bindgen_prelude::{
    Array, Either, FromNapiValue, Function, JsValue, JsValuesTupleIntoVec,
    Object, Promise, ToNapiValue, TypeName, Undefined, Unknown,
    ValidateNapiValue,
};
use napi::sys;
use napi::threadsafe_function::{ThreadsafeCallContext, ThreadsafeFunction};
use napi::{Env, Result};

use crate::_types::ast::JsAst;
use crate::_types::plugin::context::JsPluginContext;
use crate::_types::plugin::hooks::{
    JsAstArgs, JsCodeOutput, JsEndArgs, JsGenerateArgs, JsParseArgs,
    JsParseOutput, JsResolvedOptions, JsStartArgs, JsTransformBatchArgs,
    JsTransformOutput,
};
use crate::_types::refs::{SharedRef, SharedStr};
use crate::_types::sourcemap::JsSourceMap;

/// A TSFN bridging one JS hook.
///
/// - `Data`: worker-side call data moved to the JS thread.
/// - `Payload`: JS value or values the hook receives.
/// - `Return`: JS value the hook returns directly (sync) or resolves (async).
///   The hook aliases wrap it in an `Either` over `Promise<Return>`.
///
/// `CalleeHandled = false` passes hook payloads to the JS function as-is. The
/// `true` variant would prepend a Node-style error argument and shift every
/// hook parameter by one. The bridge still captures a JS throw inside the
/// hook. `call_async_catch` uses the `WithCallback` call variant, so the
/// exception reaches the awaiting worker as `Err(napi::Error)` with status
/// `PendingException`. The compile promise then rejects with the original
/// message instead of letting the error escape as a fatal exception.
pub type Tsfn<Data, Payload, Return> =
    ThreadsafeFunction<Data, Return, Payload, napi::Status, false, false, 0>;

macro_rules! for_each_hook_slot {
    ($macro_name:ident $(, $extra:tt)*) => {
        $macro_name! {
            $($extra,)*
            {
                (options, Options, crate::plugin::hooks::OptionsCall, Option<crate::_types::plugin::hooks::JsOptionsOutput>, 0),
                (start, Start, crate::plugin::hooks::FnCtx<crate::_types::plugin::hooks::JsStartArgs>, napi::bindgen_prelude::Undefined, 1),
                (parse, Parse, crate::plugin::hooks::FnCtx<crate::_types::plugin::hooks::JsParseArgs>, Option<crate::_types::plugin::hooks::JsParseOutput>, 2),
                (prepare, Prepare, crate::plugin::hooks::FnCtx<crate::_types::plugin::hooks::JsAstArgs>, napi::bindgen_prelude::Undefined, 3),
                (transform, Transform, crate::plugin::hooks::FnCtx<crate::_types::plugin::hooks::JsAstArgs>, Option<crate::_types::plugin::hooks::JsTransformOutput>, 4),
                (finalize, Finalize, crate::plugin::hooks::FnCtx<crate::_types::plugin::hooks::JsAstArgs>, napi::bindgen_prelude::Undefined, 5),
                (generate, Generate, crate::plugin::hooks::FnCtx<crate::_types::plugin::hooks::JsGenerateArgs>, Option<crate::_types::plugin::hooks::JsCodeOutput>, 6),
                (end, End, crate::plugin::hooks::FnCtx<crate::_types::plugin::hooks::JsEndArgs>, napi::bindgen_prelude::Undefined, 7),
            }
        }
    };
}

macro_rules! hook_slots {
    ($rows:tt) => {
        hook_slots_rows!($rows);
    };
}

macro_rules! hook_slots_rows {
    ({ $(($name:ident, $Name:ident, $args:ty, $ret:ty, $slot:literal)),* $(,)? }) => {
        $( // Declarative slot metadata. The `options` hook's `Args` is the
           // `OptionsCall` payload producer.
            #[allow(dead_code)]
            pub mod $name {
            pub const SLOT: usize = $slot;

            pub type Args = $args;

            pub type Return = $ret;
        } )*
    };
}

for_each_hook_slot!(hook_slots);

/// Worker-side call data for the `options` hook.
pub struct OptionsCall {
    /// Working directory used to resolve relative paths. Absent when unset.
    pub cwd: Option<SharedStr>,
    /// The file being parsed or generated. Absent when unset.
    pub file: Option<SharedStr>,
    /// Source language of the file. The wire value is `""` when unspecified.
    pub language: SharedStr,
    /// Module system used for code generation. The wire value is `""` when
    /// unspecified.
    pub source_type: SharedStr,
    /// The parser-stage options requested so far, as a JS object. `None`
    /// means unspecified, and Telarel omits it from the payload.
    pub parser: Option<serde_json::Value>,
    /// The generator-stage options requested so far, as a JS object. `None`
    /// means unspecified, and Telarel omits it from the payload.
    pub generator: Option<serde_json::Value>,
    /// The output code options requested so far, as a JS object. `None`
    /// means unspecified, and Telarel omits it from the payload.
    pub output: Option<serde_json::Value>,
    /// The source-map inputs and outputs requested so far, as the JS union.
    /// `None` means unspecified, and Telarel omits it from the payload.
    pub source_map: Option<serde_json::Value>,
    /// The settled plugin descriptors of the options bag, snapshotted at call
    /// time. The view starts with the entry descriptors and grows by each
    /// patch's additions, so every later hook sees the full settled list.
    pub plugins: Vec<SharedRef>,
}

impl ToNapiValue for OptionsCall {
    unsafe fn to_napi_value(
        env: sys::napi_env,
        val: Self,
    ) -> Result<sys::napi_value> {
        let env: Env = Env::from_raw(env);

        let mut object: Object<'static> = Object::new(&env)?;

        // `cwd` and `file` are omitted when unset so an echoed bag keeps
        // the resolution defaults. `language` and `sourceType` always ship;
        // an empty label reads as unset on the JS side.
        if let Some(cwd) = val.cwd {
            object.set("cwd", cwd)?;
        }

        if let Some(file) = val.file {
            object.set("file", file)?;
        }

        object.set("language", val.language)?;
        object.set("sourceType", val.source_type)?;

        // An omitted stage group means "unspecified" on the wire. Telarel
        // transmits only a requested group.
        if let Some(parser) = val.parser {
            object.set("parser", parser)?;
        }

        if let Some(generator) = val.generator {
            object.set("generator", generator)?;
        }

        // An omitted `output` means "unspecified" on the wire. Telarel
        // transmits only a requested output.
        if let Some(output) = val.output {
            object.set("output", output)?;
        }

        // An omitted `sourceMap` means "unspecified" on the wire. Telarel
        // transmits only a requested map.
        if let Some(source_map) = val.source_map {
            object.set("sourceMap", source_map)?;
        }

        let descriptors: Vec<Object<'static>> = val
            .plugins
            .iter()
            .map(|reference| reference.get(&env))
            .collect::<Result<Vec<Object<'static>>>>()?;

        let plugins: Array<'_> = Array::from_vec(
            &env,
            descriptors.iter().collect::<Vec<&Object<'static>>>(),
        )?;

        object.set("plugins", plugins)?;

        Ok(JsValue::raw(&object))
    }
}

/// The raw return value of the TSFN that bridges an `options` hook.
///
/// The raw `options` hook is an async function that returns
/// `Promise<Option<JsOptionsOutput>>`. The TSFN delivers the raw promise
/// object to the JS-thread callback, and the callback wires the resolution
/// bridging.
pub type OptionsReturn = Unknown<'static>;

/// TSFN bridging the `options` hook.
///
/// [`OptionsCall`] (the slot table's `Args`) emits its payload, whose
/// `plugins` property is the settled plugin descriptors of the options bag.
/// The raw `options` hook is async and returns a promise that carries
/// `Option<JsOptionsOutput>`. The TSFN delivers that promise raw. See
/// [`crate::plugin::hooks::OptionsReturn`].
pub type OptionsTsfn =
    Tsfn<OptionsCall, OptionsCall, crate::plugin::hooks::OptionsReturn>;

/// A worker-side hook call that carries a plugin context.
pub trait HookCall: 'static {
    /// JS args payload that accompanies the context.
    type Payload: ToNapiValue;

    /// JS return type of the hook, before the sync or async `Either`.
    type Return: FromNapiValue + TypeName + ValidateNapiValue + 'static;

    /// Consume the call data into the JS-facing plugin context and args
    /// payload.
    fn into_payload(self) -> Result<(JsPluginContext, Self::Payload)>;
}

/// Build the JS-facing plugin context for a hook call.
pub fn plugin_context(
    cwd: SharedStr,
    file: SharedStr,
    language: SharedStr,
    source_type: SharedStr,
) -> Result<JsPluginContext> {
    Ok(JsPluginContext { cwd, file, language, source_type })
}

/// A hook payload: the plugin context, then the hook args.
pub struct FnCtx<A>(
    /// Plugin context passed as the first hook argument.
    pub JsPluginContext,
    /// Hook args passed as the second hook argument.
    pub A,
);

impl<A: ToNapiValue> JsValuesTupleIntoVec for FnCtx<A> {
    // `env` is a raw napi handle that the JS thread owns. The trait method is
    // not `unsafe`, matching napi's own `JsValuesTupleIntoVec` impls.
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    fn into_vec(
        self,
        env: sys::napi_env,
    ) -> Result<Vec<sys::napi_value>> {
        Ok(vec![unsafe { ToNapiValue::to_napi_value(env, self.0)? }, unsafe {
            ToNapiValue::to_napi_value(env, self.1)?
        }])
    }
}

/// Worker-side call data for the `start` hook: the resolved, read-only
/// options with the settled plugin-name list.
pub struct StartCall {
    /// Working directory used to resolve relative paths.
    pub cwd: SharedStr,
    /// The file being parsed or generated.
    pub file: SharedStr,
    /// Source language of the file.
    pub language: SharedStr,
    /// Module system used for code generation.
    pub source_type: SharedStr,
    /// Options resolved for the hook or stage.
    pub options: JsResolvedOptions,
}

impl HookCall for StartCall {
    type Payload = JsStartArgs;
    type Return = Undefined;

    fn into_payload(self) -> Result<(JsPluginContext, Self::Payload)> {
        let context: JsPluginContext = plugin_context(
            self.cwd.clone(),
            self.file.clone(),
            self.language.clone(),
            self.source_type.clone(),
        )?;

        let payload: JsStartArgs = JsStartArgs { options: self.options };

        Ok((context, payload))
    }
}

/// TSFN bridging the `start` hook.
pub type StartTsfn =
    Tsfn<StartCall, start::Args, Either<Promise<start::Return>, start::Return>>;

/// Worker-side call data for the `parse` hook: the code to parse and the
/// resolved parser options.
pub struct ParseCall {
    /// Working directory used to resolve relative paths.
    pub cwd: SharedStr,
    /// The file being parsed or generated.
    pub file: SharedStr,
    /// Source code text.
    pub code: String,
    /// Source language of the file.
    pub language: SharedStr,
    /// Module system used for code generation.
    pub source_type: SharedStr,
    /// Options resolved for the hook or stage.
    pub options: crate::_types::plugin::hooks::JsParserHookOptions,
}

impl HookCall for ParseCall {
    type Payload = JsParseArgs;
    type Return = Option<JsParseOutput>;

    fn into_payload(self) -> Result<(JsPluginContext, Self::Payload)> {
        let context: JsPluginContext = plugin_context(
            self.cwd,
            self.file,
            self.language,
            self.source_type,
        )?;

        let payload: JsParseArgs =
            JsParseArgs { code: self.code, options: self.options };

        Ok((context, payload))
    }
}

/// TSFN bridging the `parse` hook.
pub type ParseTsfn =
    Tsfn<ParseCall, parse::Args, Either<Promise<parse::Return>, parse::Return>>;

/// Worker-side call data for the `prepare` and `finalize` observers.
pub struct ObserveCall {
    /// Working directory used to resolve relative paths.
    pub cwd: SharedStr,
    /// The file being parsed or generated.
    pub file: SharedStr,
    /// Source language of the file.
    pub language: SharedStr,
    /// Module system used for code generation.
    pub source_type: SharedStr,
    /// The AST as a JSON string.
    pub ast_json: String,
    /// Source text the program's spans anchor to.
    pub code: String,
    /// The input source map, when the AST carries one.
    pub map: Option<JsSourceMap>,
    /// Grammar of the AST.
    pub ast_language: String,
}

impl HookCall for ObserveCall {
    type Payload = JsAstArgs;
    type Return = Undefined;

    fn into_payload(self) -> Result<(JsPluginContext, Self::Payload)> {
        let context: JsPluginContext = plugin_context(
            self.cwd,
            self.file,
            self.language,
            self.source_type,
        )?;

        let payload: JsAstArgs = JsAstArgs {
            ast_json: self.ast_json,
            code: self.code,
            map: self.map,
            language: self.ast_language,
        };

        Ok((context, payload))
    }
}

/// TSFN bridging the `prepare` hook.
pub type PrepareTsfn = Tsfn<
    ObserveCall,
    prepare::Args,
    Either<Promise<prepare::Return>, prepare::Return>,
>;

/// TSFN bridging the `finalize` hook.
pub type FinalizeTsfn = Tsfn<
    ObserveCall,
    finalize::Args,
    Either<Promise<finalize::Return>, finalize::Return>,
>;

/// Worker-side call data for the `transform` hook.
pub struct TransformCall {
    /// Working directory used to resolve relative paths.
    pub cwd: SharedStr,
    /// The file being parsed or generated.
    pub file: SharedStr,
    /// Source language of the file.
    pub language: SharedStr,
    /// Module system used for code generation.
    pub source_type: SharedStr,
    /// The AST as a JSON string.
    pub ast_json: String,
    /// Source text the program's spans anchor to.
    pub code: String,
    /// The input source map, when the AST carries one.
    pub map: Option<JsSourceMap>,
    /// Grammar of the AST the hook receives.
    pub ast_language: String,
}

impl HookCall for TransformCall {
    type Payload = JsAstArgs;
    type Return = Option<JsTransformOutput>;

    fn into_payload(self) -> Result<(JsPluginContext, Self::Payload)> {
        let context: JsPluginContext = plugin_context(
            self.cwd,
            self.file,
            self.language,
            self.source_type,
        )?;

        let payload: JsAstArgs = JsAstArgs {
            ast_json: self.ast_json,
            code: self.code,
            map: self.map,
            language: self.ast_language,
        };

        Ok((context, payload))
    }
}

/// TSFN bridging the `transform` hook.
pub type TransformTsfn = Tsfn<
    TransformCall,
    transform::Args,
    Either<Promise<transform::Return>, transform::Return>,
>;

/// Worker-side call data for a batched `transform` run.
pub struct TransformBatchCall {
    /// Working directory used to resolve relative paths.
    pub cwd: SharedStr,
    /// The file being parsed or generated.
    pub file: SharedStr,
    /// Source language of the file.
    pub language: SharedStr,
    /// Module system used for code generation.
    pub source_type: SharedStr,
    /// The AST as a JSON string.
    pub ast_json: String,
    /// Source text the program's spans anchor to.
    pub code: String,
    /// The input source map, when the AST carries one.
    pub map: Option<JsSourceMap>,
    /// Grammar of the AST the batch receives.
    pub ast_language: String,
    /// The raw plugin descriptors for this run, in driver order.
    pub plugins: Vec<SharedRef>,
}

impl HookCall for TransformBatchCall {
    type Payload = JsTransformBatchArgs;
    type Return = Option<JsTransformOutput>;

    fn into_payload(self) -> Result<(JsPluginContext, Self::Payload)> {
        let context: JsPluginContext = plugin_context(
            self.cwd,
            self.file,
            self.language,
            self.source_type,
        )?;

        let payload: JsTransformBatchArgs = JsTransformBatchArgs {
            ast_json: self.ast_json,
            code: self.code,
            map: self.map,
            language: self.ast_language,
            plugins: self.plugins,
        };

        Ok((context, payload))
    }
}

/// A TSFN bridging one JS hook that carries a context. It carries the
/// worker-side call data `C`, the `FnCtx<C::Payload>` JS payload, and the sync
/// or async `Either` return.
pub type CtxTsfn<C> = Tsfn<
    C,
    FnCtx<<C as HookCall>::Payload>,
    Either<Promise<<C as HookCall>::Return>, <C as HookCall>::Return>,
>;

/// TSFN bridging one batched `transform` run.
pub type TransformBatchTsfn = CtxTsfn<TransformBatchCall>;

/// Worker-side call data for the `generate` hook.
pub struct GenerateCall {
    /// Working directory used to resolve relative paths.
    pub cwd: SharedStr,
    /// The file being parsed or generated.
    pub file: SharedStr,
    /// Source language of the file.
    pub language: SharedStr,
    /// Module system used for code generation.
    pub source_type: SharedStr,
    /// The AST as a normalized object.
    pub ast: JsAst,
    /// Options resolved for the hook or stage.
    pub options: crate::_types::plugin::hooks::JsGeneratorHookOptions,
}

impl HookCall for GenerateCall {
    type Payload = JsGenerateArgs;
    type Return = Option<JsCodeOutput>;

    fn into_payload(self) -> Result<(JsPluginContext, Self::Payload)> {
        let context: JsPluginContext = plugin_context(
            self.cwd,
            self.file,
            self.language,
            self.source_type,
        )?;

        let payload: JsGenerateArgs =
            JsGenerateArgs { ast: self.ast, options: self.options };

        Ok((context, payload))
    }
}

/// TSFN bridging the `generate` hook.
pub type GenerateTsfn = Tsfn<
    GenerateCall,
    generate::Args,
    Either<Promise<generate::Return>, generate::Return>,
>;

/// Worker-side call data for the `end` hook: the rendered compile error, when
/// the compile failed.
pub struct EndCall {
    /// Working directory used to resolve relative paths.
    pub cwd: SharedStr,
    /// The file being parsed or generated.
    pub file: SharedStr,
    /// Source language of the file.
    pub language: SharedStr,
    /// Module system used for code generation.
    pub source_type: SharedStr,
    /// An error that an earlier hook raised.
    pub error: Option<String>,
}

impl HookCall for EndCall {
    type Payload = JsEndArgs;
    type Return = Undefined;

    fn into_payload(self) -> Result<(JsPluginContext, Self::Payload)> {
        let context: JsPluginContext = plugin_context(
            self.cwd,
            self.file,
            self.language,
            self.source_type,
        )?;

        let payload: JsEndArgs = JsEndArgs { error: self.error };

        Ok((context, payload))
    }
}

/// TSFN bridging the `end` hook.
pub type EndTsfn =
    Tsfn<EndCall, end::Args, Either<Promise<end::Return>, end::Return>>;

/// Scan a hook slot that carries a context on a JS plugin object. Bridge the
/// present function into a TSFN.
pub fn scan_hook<C>(
    object: &Object<'static>,
    name: &str,
) -> Result<Option<CtxTsfn<C>>>
where
    C: HookCall,
{
    let Some(function) = object.get::<Function<
        '_,
        FnCtx<C::Payload>,
        Either<Promise<C::Return>, C::Return>,
    >>(name)?
    else {
        return Ok(None);
    };

    let tsfn: CtxTsfn<C> = function
        .build_threadsafe_function::<C>()
        .build_callback(move |callback: ThreadsafeCallContext<C>| {
            let (context, payload): (JsPluginContext, C::Payload) =
                callback.value.into_payload()?;

            Ok(FnCtx(context, payload))
        })?;

    Ok(Some(tsfn))
}

/// Scan the `options` hook slot on a JS plugin object and bridge its function
/// into a TSFN.
///
/// The `options` hook runs before any context exists, so its payload is the
/// `OptionsCall` data itself. That payload carries the rooted plugin array
/// instead of a context.
pub fn scan_options(object: &Object<'static>) -> Result<Option<OptionsTsfn>> {
    let Some(function) = object.get::<Function<
        '_,
        OptionsCall,
        crate::plugin::hooks::OptionsReturn,
    >>("options")?
    else {
        return Ok(None);
    };

    let tsfn: OptionsTsfn =
        function.build_threadsafe_function::<OptionsCall>().build_callback(
            |callback: ThreadsafeCallContext<OptionsCall>| Ok(callback.value),
        )?;

    Ok(Some(tsfn))
}

/// Scan every hook slot of a JS plugin object, and bridge each present hook
/// function into a TSFN.
///
/// The macro returns `(options, start, parse, prepare, transform, finalize,
/// generate, end)`, in pipeline execution order.
macro_rules! hook_scan {
    ($object:expr) => {{
        let options: Option<crate::plugin::hooks::OptionsTsfn> =
            crate::plugin::hooks::scan_options($object)?;

        let start: Option<crate::plugin::hooks::StartTsfn> =
            crate::plugin::hooks::scan_hook::<crate::plugin::hooks::StartCall>(
                $object, "start",
            )?;

        let parse: Option<crate::plugin::hooks::ParseTsfn> =
            crate::plugin::hooks::scan_hook::<crate::plugin::hooks::ParseCall>(
                $object, "parse",
            )?;

        let prepare: Option<crate::plugin::hooks::PrepareTsfn> =
            crate::plugin::hooks::scan_hook::<crate::plugin::hooks::ObserveCall>(
                $object, "prepare",
            )?;

        let transform: Option<crate::plugin::hooks::TransformTsfn> =
            crate::plugin::hooks::scan_hook::<
                crate::plugin::hooks::TransformCall,
            >($object, "transform")?;

        let finalize: Option<crate::plugin::hooks::FinalizeTsfn> =
            crate::plugin::hooks::scan_hook::<crate::plugin::hooks::ObserveCall>(
                $object, "finalize",
            )?;

        let generate: Option<crate::plugin::hooks::GenerateTsfn> =
            crate::plugin::hooks::scan_hook::<
                crate::plugin::hooks::GenerateCall,
            >($object, "generate")?;

        let end: Option<crate::plugin::hooks::EndTsfn> =
            crate::plugin::hooks::scan_hook::<crate::plugin::hooks::EndCall>(
                $object, "end",
            )?;

        (options, start, parse, prepare, transform, finalize, generate, end)
    }};
}

pub(crate) use hook_scan;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hook_slots_are_sequential() {
        assert_eq!(options::SLOT, 0);
        assert_eq!(start::SLOT, 1);
        assert_eq!(parse::SLOT, 2);
        assert_eq!(prepare::SLOT, 3);
        assert_eq!(transform::SLOT, 4);
        assert_eq!(finalize::SLOT, 5);
        assert_eq!(generate::SLOT, 6);
        assert_eq!(end::SLOT, 7);
    }

    #[test]
    fn test_start_call_payload_carries_resolved_options() {
        let call: StartCall = StartCall {
            cwd: SharedStr::new("/repo"),
            file: SharedStr::new("index.ts"),
            language: SharedStr::new("ts"),
            source_type: SharedStr::new("module"),
            options: JsResolvedOptions {
                cwd: String::from("/repo"),
                file: String::from("index.ts"),
                language: String::from("ts"),
                source_type: String::from("module"),
                parser:
                    crate::_types::plugin::hooks::JsResolvedParserStageOptions {
                        preserve_parens: false,
                        allow_return_outside_function: false,
                        oxc: serde_json::json!({}),
                    },
                generator:
                    crate::_types::plugin::hooks::JsResolvedGeneratorStageOptions
                    {
                        single_quote: false,
                        ascii_only: false,
                        indent_style: String::from("space"),
                        indent_width: 2,
                        comments: true,
                        minify: false,
                        oxc: serde_json::json!({}),
                    },
                output: crate::_types::plugin::hooks::JsResolvedOutputOptions {
                    file: String::from("dist/index.js"),
                },
                source_map: serde_json::json!({
                    "input": "auto",
                    "output": "both",
                }),
                plugins: vec![String::from("observe")],
            },
        };

        let (_context, payload) = call.into_payload().expect("payload builds");

        assert_eq!(payload.options.file, "index.ts");
        assert_eq!(payload.options.cwd, "/repo");
        assert_eq!(payload.options.plugins, vec![String::from("observe")]);
        assert_eq!(
            payload.options.source_map,
            serde_json::json!({ "input": "auto", "output": "both" })
        );
    }
}
