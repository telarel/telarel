use std::borrow::Cow;

use napi::bindgen_prelude::{Either, Object, Promise, Undefined};
use napi::{Error, Result, Status};
use oxc::ast::ast::Program;
use oxc::span::SourceType;

use telarel_common::{
    CompileContext, CompileOptions, HookUsage, PartialCompileOptions,
};
use telarel_plugin::__internal::{HookFuture, LocalHookFuture};
use telarel_plugin::{
    Pluginable, PostArgs, PreArgs, TransformArgs, TransformOutput,
    TransformReturn,
};

use crate::_types::plugin::hooks::{JsOptionsOutput, JsTransformOutput};
use crate::plugin::hooks::{
    OptionsCall, OptionsTsfn, PostTsfn, PreTsfn, SharedRef, StageCall,
    TransformCall, TransformTsfn, hook_scan,
};

/// A JS plugin object bridged into the Rust plugin layer.
pub struct JsPlugin {
    name: String,
    /// Rooted JS array holding every raw plugin object (for the `options` hook payload).
    plugins: SharedRef,
    tsfn_options: Option<OptionsTsfn>,
    tsfn_pre: Option<PreTsfn>,
    tsfn_transform: Option<TransformTsfn>,
    tsfn_post: Option<PostTsfn>,
}

impl JsPlugin {
    /// Bridge a raw JS plugin object: validate `name` and scan the hook
    /// slots, wrapping present hook functions into TSFNs.
    pub fn from_object(
        object: &Object<'static>,
        metadata: &SharedRef,
        plugins: &SharedRef,
    ) -> Result<Self> {
        let name: String = object.get::<String>("name")?.ok_or_else(|| {
            Error::new(
                Status::InvalidArg,
                "plugin `name` is required".to_owned(),
            )
        })?;

        let (tsfn_options, tsfn_pre, tsfn_transform, tsfn_post) =
            hook_scan!(object, metadata);

        Ok(Self {
            name,
            plugins: plugins.clone(),
            tsfn_options,
            tsfn_pre,
            tsfn_transform,
            tsfn_post,
        })
    }
}

/// Convert a napi error raised by a hook call into an anyhow error.
fn hook_error(error: Error) -> anyhow::Error {
    anyhow::Error::from(error)
}

impl Pluginable for JsPlugin {
    fn call_name(&self) -> Cow<'static, str> {
        Cow::Owned(self.name.clone())
    }

    fn call_hook_usage(&self) -> HookUsage {
        let mut usage: HookUsage = HookUsage::default();

        if self.tsfn_options.is_some() {
            usage.insert(HookUsage::OPTIONS);
        }

        if self.tsfn_pre.is_some() {
            usage.insert(HookUsage::PRE);
        }

        if self.tsfn_transform.is_some() {
            usage.insert(HookUsage::TRANSFORM);
        }

        if self.tsfn_post.is_some() {
            usage.insert(HookUsage::POST);
        }

        usage
    }

    fn call_options<'a>(
        &'a self,
        options: &'a CompileOptions,
    ) -> HookFuture<'a, anyhow::Result<Option<PartialCompileOptions>>> {
        Box::pin(async move {
            let Some(tsfn) = self.tsfn_options.as_ref() else {
                return Ok(None);
            };

            let call: OptionsCall = OptionsCall {
                cwd: options.cwd.clone(),
                file: options.file.clone(),
                code: options.code.clone(),
                plugins: self.plugins.clone(),
            };

            let output: Either<
                Promise<Option<JsOptionsOutput>>,
                Option<JsOptionsOutput>,
            > = tsfn.call_async_catch(call).await.map_err(hook_error)?;

            let replaced: Option<JsOptionsOutput> = match output {
                | Either::A(promise) => promise.await.map_err(hook_error)?,
                | Either::B(value) => value,
            };

            Ok(replaced.map(|output| PartialCompileOptions {
                cwd: output.cwd,
                file: output.file,
                code: output.code,
            }))
        })
    }

    fn call_pre<'a>(
        &'a self,
        ctx: &'a CompileContext<'_>,
        args: &'a PreArgs<'_>,
    ) -> HookFuture<'a, anyhow::Result<()>> {
        Box::pin(async move {
            let Some(tsfn) = self.tsfn_pre.as_ref() else {
                return Ok(());
            };

            let call: StageCall = StageCall {
                cwd: ctx.cwd.to_string(),
                file: args.file.to_string(),
                code: args.code.to_string(),
            };

            let output: Either<Promise<Undefined>, Undefined> =
                tsfn.call_async_catch(call).await.map_err(hook_error)?;

            if let Either::A(promise) = output {
                promise.await.map_err(hook_error)?;
            }

            Ok(())
        })
    }

    fn call_transform<'a>(
        &'a self,
        ctx: &'a CompileContext<'a>,
        args: &'a TransformArgs<'a>,
    ) -> LocalHookFuture<'a, TransformReturn<'a>> {
        Box::pin(async move {
            let Some(tsfn) = self.tsfn_transform.as_ref() else {
                return Ok(None);
            };

            // Capture the source type BEFORE serializing; the read-back needs
            // it to anchor the JSON tree.
            let source_type: SourceType = args.program.source_type;

            let ast_json: String = oxc_estree_codec::program_to_json(
                args.program,
                oxc_estree_codec::ProgramToJsonOptions::new(),
            );

            let call: TransformCall = TransformCall {
                cwd: ctx.cwd.to_string(),
                file: args.file.to_string(),
                code: ctx.code.to_string(),
                ast_json,
            };

            let output: Either<
                Promise<Option<JsTransformOutput>>,
                Option<JsTransformOutput>,
            > = tsfn.call_async_catch(call).await.map_err(hook_error)?;

            let replaced: Option<JsTransformOutput> = match output {
                | Either::A(promise) => promise.await.map_err(hook_error)?,
                | Either::B(value) => value,
            };

            let Some(replaced) = replaced else {
                return Ok(None);
            };

            // Read the replacement tree back into the compile allocator; the
            // original code anchors the spans.
            let program: Program<'_> = oxc_estree_codec::json_to_program(
                &replaced.ast_json,
                oxc_estree_codec::JsonToProgramOptions {
                    allocator: args.allocator,
                    source_type,
                    source_text: ctx.code,
                },
            )
            .map_err(anyhow::Error::from)?;

            Ok(Some(TransformOutput { program }))
        })
    }

    fn call_post<'a>(
        &'a self,
        ctx: &'a CompileContext<'_>,
        args: &'a PostArgs<'_>,
    ) -> HookFuture<'a, anyhow::Result<()>> {
        Box::pin(async move {
            let Some(tsfn) = self.tsfn_post.as_ref() else {
                return Ok(());
            };

            let call: StageCall = StageCall {
                cwd: ctx.cwd.to_string(),
                file: args.file.to_string(),
                code: args.code.to_string(),
            };

            let output: Either<Promise<Undefined>, Undefined> =
                tsfn.call_async_catch(call).await.map_err(hook_error)?;

            if let Either::A(promise) = output {
                promise.await.map_err(hook_error)?;
            }

            Ok(())
        })
    }
}
