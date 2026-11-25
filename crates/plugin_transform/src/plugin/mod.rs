use std::borrow::Cow;
use std::path::Path;
use std::sync::OnceLock;

use oxc::semantic::SemanticBuilder;
use oxc::transformer::{TransformOptions as OxcTransformOptions, Transformer};
use oxc_transformer_plugins::{InjectGlobalVariables, ReplaceGlobalDefines};

use telarel_common::{Ast, HookUsage, generate_program};
use telarel_plugin::{
    CommonPluginContext, OptionsArgs, OptionsReturn, Plugin, PluginContext,
    PluginError, PluginHookMeta, PluginOrder, TransformArgs, TransformOutput,
};

use crate::options::TransformOptions;
use crate::options::helper_loader::ResolvedHelpers;

/// The default transform plugin name on the Rust side.
pub const NAME: &str = "telarel_plugin_transform";

#[derive(Debug, Clone)]
struct Resolved {
    oxc: OxcTransformOptions,
    helpers: ResolvedHelpers,
    define: Option<oxc_transformer_plugins::ReplaceGlobalDefinesConfig>,
    inject: Option<oxc_transformer_plugins::InjectGlobalVariablesConfig>,
}

fn render_diagnostics(
    diagnostics: &oxc::diagnostics::Diagnostics
) -> PluginError {
    let rendered: String = diagnostics
        .errors()
        .map(oxc::diagnostics::OxcDiagnostic::render)
        .collect::<Vec<String>>()
        .join("\n");

    PluginError::new(rendered)
}

/// The transform plugin. It wraps the oxc transformer and runs the transform
/// passes.
#[derive(Debug)]
pub struct TransformPlugin {
    name: Cow<'static, str>,
    options: TransformOptions,
    /// Lazily resolved oxc options and plugin configuration.
    /// An `Err` caches the resolution failure and replays it on every call.
    resolved: OnceLock<Result<Resolved, PluginError>>,
}

impl TransformPlugin {
    /// Create a plugin with default options.
    pub fn new() -> Self {
        Self::with_options(TransformOptions::default())
    }

    /// Create a plugin with `options`.
    pub fn with_options(options: TransformOptions) -> Self {
        Self { name: Cow::Borrowed(NAME), options, resolved: OnceLock::new() }
    }

    /// Override the name reported to the plugin pipeline.
    pub fn with_name(
        mut self,
        name: impl Into<Cow<'static, str>>,
    ) -> Self {
        self.name = name.into();
        self
    }

    async fn transform_inner<'a>(
        &'a self,
        ctx: &'a PluginContext<'_>,
        args: TransformArgs,
    ) -> Result<Option<TransformOutput>, PluginError> {
        // The cache is a pure function of the options. The per-call cwd
        // overlays a clone so the cache stays cwd-free.
        let resolved: &Result<Resolved, PluginError> =
            self.resolved.get_or_init(|| {
                let define: Result<
                    Option<oxc_transformer_plugins::ReplaceGlobalDefinesConfig>,
                    oxc::diagnostics::Diagnostics,
                > = self.options.resolve_define();

                let define: Option<
                    oxc_transformer_plugins::ReplaceGlobalDefinesConfig,
                > = match define {
                    | Ok(define) => define,
                    | Err(diagnostics) => {
                        return Err(render_diagnostics(&diagnostics));
                    },
                };

                let oxc: OxcTransformOptions = self.options.resolve();

                if let Err(message) = self.options.validate(&oxc) {
                    return Err(PluginError::new(message));
                }

                Ok(Resolved {
                    oxc,
                    helpers: self.options.resolve_helpers(),
                    define,
                    inject: self.options.resolve_inject(),
                })
            });

        // A cached resolution error replays on every call.
        // The program is unmodified because resolution precedes the transformer.
        let resolved: &Resolved = match resolved {
            | Ok(resolved) => resolved,
            | Err(error) => return Err(error.clone()),
        };

        let mut resolved_oxc: OxcTransformOptions = resolved.oxc.clone();

        TransformOptions::apply_cwd(&mut resolved_oxc, ctx.cwd);

        // The driver retains the carried AST, so `with_mut` copies on write.
        // The copy is discarded when the passes report no change.
        let mut ast: Ast = args.ast;

        // The main oxc transformer has no changed flag. Compare code generation
        // before and after the pass instead. The inject and define passes report
        // their own `changed` flag, and the helpers pass reports whether it edited
        // the program.
        let before: String = generate_program(ast.program(), ctx.module.file).0;

        let mut changed: bool = false;

        ast.with_mut(|allocator, program| -> Result<(), PluginError> {
            let scoping: oxc::semantic::Scoping = SemanticBuilder::new()
                .with_excess_capacity(2.0)
                .with_enum_eval(true)
                .build(program)
                .semantic
                .into_scoping();

            let transformer_return: oxc::transformer::TransformerReturn =
                Transformer::new(
                    allocator,
                    Path::new(ctx.module.file),
                    &resolved_oxc,
                )
                .build_with_scoping(scoping, program);

            let diagnostics: oxc::diagnostics::Diagnostics =
                transformer_return.diagnostics;

            if diagnostics.has_errors() {
                return Err(render_diagnostics(&diagnostics));
            }

            changed = changed
                || generate_program(program, ctx.module.file).0 != before;

            // Replace oxc runtime helper imports with the vendored bodies
            // before inject and define run.
            if resolved.helpers.inline() {
                let inlined: bool = crate::helpers::run(
                    allocator,
                    program,
                    &resolved.helpers.module_name,
                )?;

                changed = changed || inlined;
            }

            let inject_configured: bool = resolved.inject.is_some();

            let define_configured: bool = resolved.define.is_some();

            if inject_configured || define_configured {
                let mut scoping: oxc::semantic::Scoping = SemanticBuilder::new()
                    .with_excess_capacity(2.0)
                    .with_enum_eval(true)
                    .build(program)
                    .semantic
                    .into_scoping();

                if let Some(inject) = &resolved.inject {
                    let inject_return: oxc_transformer_plugins::InjectGlobalVariablesReturn =
                        InjectGlobalVariables::new(allocator, inject.clone())
                            .build(scoping, program);

                    scoping = inject_return.scoping;

                    changed = changed || inject_return.changed;
                }

                if let Some(define) = &resolved.define {
                    let define_return: oxc_transformer_plugins::ReplaceGlobalDefinesReturn =
                        ReplaceGlobalDefines::new(allocator, define.clone())
                            .build(scoping, program);

                    changed = changed || define_return.changed;
                }
            }

            Ok(())
        })?;

        if !changed {
            return Ok(None);
        }

        Ok(Some(TransformOutput { ast }))
    }
}

impl Default for TransformPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for TransformPlugin {
    fn name(&self) -> Cow<'static, str> {
        self.name.clone()
    }

    fn register_hook_usage(&self) -> HookUsage {
        HookUsage::Transform
    }

    async fn options<'a>(
        &'a self,
        _ctx: &'a CommonPluginContext,
        args: &'a OptionsArgs,
    ) -> OptionsReturn {
        // A `Post` options hook sees the final `file` and `language` chosen by
        // the normal hooks.
        let Some(output_file): Option<String> =
            self.options.output_file(&args.options)
        else {
            return Ok(None);
        };

        let mut next: OptionsArgs = args.clone();

        next.options.output.get_or_insert_default().file = Some(output_file);

        Ok(Some(next))
    }

    fn options_meta(&self) -> Option<PluginHookMeta> {
        Some(PluginHookMeta { order: Some(PluginOrder::Post) })
    }

    async fn transform<'a>(
        &'a self,
        ctx: &'a PluginContext<'_>,
        args: TransformArgs,
    ) -> telarel_plugin::TransformReturn {
        self.transform_inner(ctx, args).await
    }
}

#[cfg(test)]
mod tests;
