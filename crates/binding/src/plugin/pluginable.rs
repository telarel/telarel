use std::borrow::Cow;

use napi::bindgen_prelude::{Either, Object, Promise, Undefined};
use napi::{Error, Result, Status};
use oxc::ast::ast::Program;
use oxc::span::SourceType;

use telarel_common::{CompileContext, CompileOptions, HookUsage};
use telarel_plugin::__internal::{HookFuture, LocalHookFuture};
use telarel_plugin::{
    Pluginable, PostArgs, PreArgs, TransformArgs, TransformReturn,
};

use crate::_types::plugin::hooks::{JsOptionsOutput, JsTransformOutput};
use crate::plugin::build::NAME_REQUIRED;
use crate::plugin::hooks::{
    OptionsCall, OptionsTsfn, PostTsfn, PreTsfn, SharedRef, SharedStr,
    StageCall, TransformCall, TransformTsfn, hook_scan,
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
        plugins: &SharedRef,
    ) -> Result<Self> {
        let name: String = object.get::<String>("name")?.ok_or_else(|| {
            Error::new(Status::InvalidArg, NAME_REQUIRED.to_owned())
        })?;

        let (tsfn_options, tsfn_pre, tsfn_transform, tsfn_post) =
            hook_scan!(object);

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

    fn call_register_hook_usage(&self) -> HookUsage {
        let mut usage: HookUsage = HookUsage::default();

        if self.tsfn_options.is_some() {
            usage.insert(HookUsage::Options);
        }

        if self.tsfn_pre.is_some() {
            usage.insert(HookUsage::Pre);
        }

        if self.tsfn_transform.is_some() {
            usage.insert(HookUsage::Transform);
        }

        if self.tsfn_post.is_some() {
            usage.insert(HookUsage::Post);
        }

        usage
    }

    fn call_options<'a>(
        &'a self,
        options: &'a mut CompileOptions,
    ) -> HookFuture<'a, anyhow::Result<()>> {
        Box::pin(async move {
            let Some(tsfn) = self.tsfn_options.as_ref() else {
                return Ok(());
            };

            let call: OptionsCall = OptionsCall {
                cwd: SharedStr::new(options.cwd.as_deref().unwrap_or("")),
                file: SharedStr::new(&options.file),
                code: SharedStr::new(&options.code),
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

            let Some(output) = replaced else {
                return Ok(());
            };

            options.cwd = Some(output.cwd);
            options.file = output.file;
            options.code = output.code;

            Ok(())
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
                cwd: SharedStr::new(ctx.cwd),
                file: SharedStr::new(args.file),
                code: SharedStr::new(args.code),
            };

            let output: Either<Promise<Undefined>, Undefined> =
                tsfn.call_async_catch(call).await.map_err(hook_error)?;

            if let Either::A(promise) = output {
                promise.await.map_err(hook_error)?;
            }

            Ok(())
        })
    }

    fn call_transform<'a, 'ast>(
        &'a self,
        ctx: &'a CompileContext<'a>,
        args: TransformArgs<'a, 'ast>,
    ) -> LocalHookFuture<'a, TransformReturn> {
        Box::pin(async move {
            let Some(tsfn) = self.tsfn_transform.as_ref() else {
                return Ok(());
            };

            // Capture the source type BEFORE serializing; the read-back needs
            // it to anchor the JSON tree.
            let source_type: SourceType = args.program.source_type;

            let ast_json: String = oxc_estree_codec::program_to_json(
                &*args.program,
                oxc_estree_codec::ProgramToJsonOptions::new(),
            );

            let call: TransformCall = TransformCall {
                cwd: SharedStr::new(ctx.cwd),
                file: SharedStr::new(args.file),
                code: SharedStr::new(ctx.code),
                ast_json: ast_json.clone(),
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
                // The JS wrapper re-stringified the tree and compared it with
                // the baseline; `null` means the tree did not change.
                return Ok(());
            };

            // Defense in depth: a raw (unwrapped) plugin may send the tree
            // back unchanged; skip the read-back in that case too.
            if replaced.ast_json == ast_json {
                return Ok(());
            }

            // Root the source text in the compile allocator so the read-back
            // tree's data lifetime matches the allocator's, not the hook
            // call's; the original code anchors the spans.
            let source: &str = args.allocator.alloc_str(ctx.code);

            let program: Program<'_> = oxc_estree_codec::json_to_program(
                &replaced.ast_json,
                oxc_estree_codec::JsonToProgramOptions {
                    allocator: args.allocator,
                    source_type,
                    source_text: source,
                },
            )
            .map_err(anyhow::Error::from)?;

            *args.program = program;

            Ok(())
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
                cwd: SharedStr::new(ctx.cwd),
                file: SharedStr::new(args.file),
                code: SharedStr::new(args.code),
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
