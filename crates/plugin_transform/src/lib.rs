//! # Telarel Plugin Transform
//!
//! A transform plugin for the compiler.
//!
//! This crate implements [`telarel_plugin::Plugin`] over the oxc transformer:
//! it resolves telarel-owned options into oxc [`TransformOptions`], then runs
//! the TypeScript, JSX, and syntax-lowering pipeline on the program in place.

mod options;

use std::borrow::Cow;
use std::path::Path;
use std::sync::OnceLock;

use oxc::semantic::SemanticBuilder;
use oxc::transformer::{TransformOptions as OxcTransformOptions, Transformer};

use telarel_common::HookUsage;
use telarel_plugin::{Plugin, TransformArgs, TransformReturn};

pub use options::{
    JsxOptions, JsxRuntime, TransformOptions, TransformTarget,
    TypeScriptOptions,
};

/// The builtin transform plugin name.
pub const NAME: &str = "builtin:transform";

/// The builtin transform plugin, wrapping the oxc transformer.
#[derive(Debug)]
pub struct TransformPlugin {
    /// Transform options.
    options: TransformOptions,
    /// Lazily resolved oxc options.
    ///
    /// `TransformOptions` is immutable after construction, so the cache never goes stale.
    resolved: OnceLock<OxcTransformOptions>,
}

impl TransformPlugin {
    /// Create a plugin with default options.
    pub fn new() -> Self {
        Self { options: TransformOptions::default(), resolved: OnceLock::new() }
    }

    /// Create a plugin with `options`.
    pub fn with_options(options: TransformOptions) -> Self {
        Self { options, resolved: OnceLock::new() }
    }
}

impl Default for TransformPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for TransformPlugin {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed(NAME)
    }

    fn register_hook_usage(&self) -> HookUsage {
        HookUsage::Transform
    }

    async fn transform<'a, 'ast>(
        &'a self,
        ctx: &'a telarel_common::CompileContext<'a>,
        args: TransformArgs<'a, 'ast>,
    ) -> TransformReturn {
        // The cached resolution is a pure function of the options; the
        // per-call `cwd` from the compile context overlays on a clone so
        // the cache itself stays cwd-free.
        let mut resolved: OxcTransformOptions =
            self.resolved.get_or_init(|| self.options.resolve()).clone();

        TransformOptions::apply_cwd(&mut resolved, ctx.cwd);

        let scoping = SemanticBuilder::new()
            .with_excess_capacity(2.0)
            .with_enum_eval(true)
            .build(args.program)
            .semantic
            .into_scoping();

        let transformer_return: oxc::transformer::TransformerReturn =
            Transformer::new(args.allocator, Path::new(args.file), &resolved)
                .build_with_scoping(scoping, args.program);

        let diagnostics: oxc::diagnostics::Diagnostics =
            transformer_return.diagnostics;

        if diagnostics.has_errors() {
            let rendered: String = diagnostics
                .errors()
                .map(oxc::diagnostics::OxcDiagnostic::render)
                .collect::<Vec<String>>()
                .join("\n");

            return Err(anyhow::anyhow!("{rendered}"));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use oxc::allocator::Allocator;
    use oxc::ast::ast::Program;
    use oxc::parser::Parser;
    use oxc::span::SourceType;

    use telarel_common::CompileContext;
    use telarel_plugin::Plugin;

    use crate::options::{
        JsxOptions, JsxRuntime, TransformOptions, TransformTarget,
    };

    use super::*;

    const CWD: &str = "";

    const FILE_TS: &str = "index.ts";

    const FILE_TSX: &str = "index.tsx";

    const SOURCE_TS: &str = "const value: number = 1;";

    const SOURCE_JSX: &str = "const element = <div className=\"x\">hi</div>;";

    const SOURCE_REGEX: &str = "const r = /(/;";

    fn parse<'a>(
        allocator: &'a Allocator,
        file: &'a str,
        code: &'a str,
    ) -> Program<'a> {
        let source_type: SourceType =
            SourceType::from_path(file).expect("known file extension");

        Parser::new(allocator, code, source_type).parse().program
    }

    async fn run_hook<'a>(
        plugin: &TransformPlugin,
        allocator: &'a Allocator,
        file: &str,
        program: &mut Program<'a>,
    ) -> TransformReturn {
        let ctx: CompileContext<'_> = CompileContext::new(CWD, file, "");

        let args: TransformArgs<'_, '_> =
            TransformArgs { allocator, file, program };

        plugin.transform(&ctx, args).await
    }

    async fn codegen_after(
        plugin: TransformPlugin,
        file: &str,
        code: &str,
    ) -> String {
        let allocator: Allocator = Allocator::default();

        let mut program: Program<'_> = parse(&allocator, file, code);

        run_hook(&plugin, &allocator, file, &mut program)
            .await
            .expect("transform succeeds");

        telarel_common::codegen(telarel_common::CodegenOptions {
            file,
            program: &program,
        })
        .code
    }

    #[test]
    fn test_default_delegates_to_new() {
        let plugin: TransformPlugin = TransformPlugin::default();

        assert_eq!(plugin.name(), NAME);
    }

    #[test]
    fn test_plugin_name() {
        let plugin: TransformPlugin = TransformPlugin::new();

        assert_eq!(plugin.name(), NAME);
    }

    #[test]
    fn test_plugin_register_hook_usage() {
        let plugin: TransformPlugin = TransformPlugin::new();

        assert!(plugin.register_hook_usage().contains(HookUsage::Transform));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_transform_strips_typescript_types() {
        let plugin: TransformPlugin = TransformPlugin::new();

        let code: String = codegen_after(plugin, FILE_TS, SOURCE_TS).await;

        assert!(code.contains("const value"), "{}", code);
        assert!(!code.contains("number"), "{}", code);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_transform_jsx_automatic_runtime() {
        let plugin: TransformPlugin =
            TransformPlugin::with_options(TransformOptions {
                jsx: Some(JsxOptions {
                    runtime: Some(JsxRuntime::Automatic),
                    ..JsxOptions::default()
                }),
                ..TransformOptions::default()
            });

        let code: String = codegen_after(plugin, FILE_TSX, SOURCE_JSX).await;

        assert!(code.contains("jsx"), "{}", code);
        assert!(!code.contains("<div"), "{}", code);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_transform_target_lowering() {
        let plugin: TransformPlugin =
            TransformPlugin::with_options(TransformOptions {
                targets: vec![TransformTarget::Es2015],
                ..TransformOptions::default()
            });

        let code: String = codegen_after(
            plugin,
            "index.js",
            "async function main() { await g(); }",
        )
        .await;

        assert!(
            code.contains("asyncToGenerator") || code.contains("function*"),
            "{}",
            code
        );
        assert!(!code.contains("async function"), "{}", code);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_transform_invalid_regexp_errors() {
        let plugin: TransformPlugin =
            TransformPlugin::with_options(TransformOptions {
                targets: vec![TransformTarget::Es2015],
                ..TransformOptions::default()
            });

        let allocator: Allocator = Allocator::default();

        let mut program: Program<'_> =
            parse(&allocator, "index.js", SOURCE_REGEX);

        let result: TransformReturn =
            run_hook(&plugin, &allocator, "index.js", &mut program).await;

        let error: anyhow::Error =
            result.expect_err("malformed regexp must error");

        assert!(error.to_string().contains("regular expression"), "{}", error);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_transform_warnings_are_non_fatal() {
        let plugin: TransformPlugin =
            TransformPlugin::with_options(TransformOptions {
                targets: vec![TransformTarget::Es2022],
                ..TransformOptions::default()
            });

        let allocator: Allocator = Allocator::default();

        let mut program: Program<'_> =
            parse(&allocator, "index.js", "const value = await promise;");

        let result: TransformReturn =
            run_hook(&plugin, &allocator, "index.js", &mut program).await;

        assert!(result.is_ok());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_transform_new_defaults_pass_through() {
        let plugin: TransformPlugin = TransformPlugin::new();

        let code: String = codegen_after(plugin, FILE_TS, SOURCE_TS).await;

        assert!(code.contains("const value"), "{}", code);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_transform_context_cwd_applied() {
        // A non-empty compile context `cwd` must overlay onto the resolved
        // oxc options; the plugin-level cache stays cwd-free.
        let plugin: TransformPlugin =
            TransformPlugin::with_options(TransformOptions {
                targets: vec![TransformTarget::Es2015],
                ..TransformOptions::default()
            });

        let allocator: Allocator = Allocator::default();

        let mut program: Program<'_> =
            parse(&allocator, "index.js", "async function main() {}");

        let cwd: &str = "/repo";

        let ctx: CompileContext<'_> = CompileContext::new(cwd, "index.js", "");

        let args: TransformArgs<'_, '_> = TransformArgs {
            allocator: &allocator,
            file: "index.js",
            program: &mut program,
        };

        plugin.transform(&ctx, args).await.expect("transform succeeds");

        let cached: &oxc::transformer::TransformOptions =
            plugin.resolved.get().expect("cached resolution");

        assert!(
            cached.cwd.as_os_str().is_empty(),
            "cache must stay cwd-free, got {:?}",
            cached.cwd
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_transform_reuses_cached_resolution() {
        let plugin: TransformPlugin =
            TransformPlugin::with_options(TransformOptions {
                targets: vec![TransformTarget::Es2015],
                ..TransformOptions::default()
            });

        let allocator: Allocator = Allocator::default();

        let mut first: Program<'_> =
            parse(&allocator, "index.js", "async function main() {}");

        run_hook(&plugin, &allocator, "index.js", &mut first)
            .await
            .expect("first transform succeeds");

        let mut second: Program<'_> =
            parse(&allocator, "index.js", "async function other() {}");

        run_hook(&plugin, &allocator, "index.js", &mut second)
            .await
            .expect("second transform succeeds");

        let first_code: String =
            telarel_common::codegen(telarel_common::CodegenOptions {
                file: "index.js",
                program: &first,
            })
            .code;

        let second_code: String =
            telarel_common::codegen(telarel_common::CodegenOptions {
                file: "index.js",
                program: &second,
            })
            .code;

        assert_eq!(
            first_code.contains("asyncToGenerator"),
            second_code.contains("asyncToGenerator")
        );
        assert!(plugin.resolved.get().is_some());
    }
}
