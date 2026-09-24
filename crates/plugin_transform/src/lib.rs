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
use oxc_transformer_plugins::{InjectGlobalVariables, ReplaceGlobalDefines};

use telarel_common::HookUsage;
use telarel_plugin::{Plugin, TransformArgs, TransformReturn};

pub use options::{
    DefineOptions, InjectEntry, InjectOptions, InjectSpecifier, JsxOptions,
    JsxRuntime, TransformOptions, TransformTarget, TypeScriptOptions,
};

/// The builtin transform plugin name.
pub const NAME: &str = "builtin:transform";

/// The resolved, cached plugin state.
#[derive(Debug, Clone)]
struct Resolved {
    /// The oxc transform options for the main transformer pass.
    oxc: OxcTransformOptions,
    /// The resolved `define` config, `None` when not configured.
    define: Option<oxc_transformer_plugins::ReplaceGlobalDefinesConfig>,
    /// The resolved `inject` config, `None` when not configured.
    inject: Option<oxc_transformer_plugins::InjectGlobalVariablesConfig>,
}

/// The builtin transform plugin, wrapping the oxc transformer.
#[derive(Debug)]
pub struct TransformPlugin {
    /// Transform options.
    options: TransformOptions,
    /// Lazily resolved oxc options and plugin configs;
    /// an `Err` caches the config resolution failure so it replays on every call.
    resolved: OnceLock<Result<Resolved, anyhow::Error>>,
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
        let resolved: &Result<Resolved, anyhow::Error> =
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
                        let rendered: String = diagnostics
                            .errors()
                            .map(oxc::diagnostics::OxcDiagnostic::render)
                            .collect::<Vec<String>>()
                            .join("\n");

                        return Err(anyhow::anyhow!("{rendered}"));
                    },
                };

                Ok(Resolved {
                    oxc: self.options.resolve(),
                    define,
                    inject: self.options.resolve_inject(),
                })
            });

        // A cached resolution error replays on every call; the program has
        // not been mutated because resolution precedes the transformer.
        let resolved: &Resolved = match resolved {
            | Ok(resolved) => resolved,
            | Err(error) => return Err(anyhow::anyhow!("{error}")),
        };

        let mut resolved_oxc: OxcTransformOptions = resolved.oxc.clone();

        TransformOptions::apply_cwd(&mut resolved_oxc, ctx.cwd);

        let scoping = SemanticBuilder::new()
            .with_excess_capacity(2.0)
            .with_enum_eval(true)
            .build(args.program)
            .semantic
            .into_scoping();

        let transformer_return: oxc::transformer::TransformerReturn =
            Transformer::new(
                args.allocator,
                Path::new(args.file),
                &resolved_oxc,
            )
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

        // Mirror oxc 0.150.0 `src/compiler.rs:194-208`: after the transformer
        // mutates the AST, the inject and define passes require in-sync
        // scoping, so it is rebuilt from the transformed program before
        // running inject (threading the returned scoping), then define.
        let inject_configured: bool = resolved.inject.is_some();

        let define_configured: bool = resolved.define.is_some();

        if inject_configured || define_configured {
            let mut scoping: oxc::semantic::Scoping = SemanticBuilder::new()
                .with_excess_capacity(2.0)
                .with_enum_eval(true)
                .build(args.program)
                .semantic
                .into_scoping();

            if inject_configured {
                let inject: &oxc_transformer_plugins::InjectGlobalVariablesConfig =
                    resolved.inject.as_ref().expect("inject configured");

                let inject_return: oxc_transformer_plugins::InjectGlobalVariablesReturn =
                    InjectGlobalVariables::new(args.allocator, inject.clone())
                        .build(scoping, args.program);

                scoping = inject_return.scoping;
            }

            if define_configured {
                let define: &oxc_transformer_plugins::ReplaceGlobalDefinesConfig =
                    resolved.define.as_ref().expect("define configured");

                let _ =
                    ReplaceGlobalDefines::new(args.allocator, define.clone())
                        .build(scoping, args.program);
            }
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
        DefineOptions, InjectEntry, InjectOptions, InjectSpecifier, JsxOptions,
        JsxRuntime, TransformOptions, TransformTarget,
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

        let cached: &oxc::transformer::TransformOptions = &plugin
            .resolved
            .get()
            .expect("cached resolution")
            .as_ref()
            .expect("resolution succeeded")
            .oxc;

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

    #[tokio::test(flavor = "current_thread")]
    async fn test_transform_inject_named_import() {
        let plugin: TransformPlugin =
            TransformPlugin::with_options(TransformOptions {
                inject: Some(InjectOptions {
                    entries: vec![InjectEntry {
                        source: String::from("jquery"),
                        specifier: InjectSpecifier::Named {
                            imported: None,
                            local: String::from("$"),
                        },
                    }],
                }),
                ..TransformOptions::default()
            });

        let code: String =
            codegen_after(plugin, "index.js", "const element = $(\"#root\");")
                .await;

        assert!(code.contains("import"), "{}", code);
        assert!(code.contains("jquery"), "{}", code);
        assert!(code.contains("$"), "{}", code);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_transform_inject_dotted_local() {
        let plugin: TransformPlugin =
            TransformPlugin::with_options(TransformOptions {
                inject: Some(InjectOptions {
                    entries: vec![InjectEntry {
                        source: String::from("object-assign-shim"),
                        specifier: InjectSpecifier::Named {
                            imported: Some(String::from("default")),
                            local: String::from("Object.assign"),
                        },
                    }],
                }),
                ..TransformOptions::default()
            });

        let code: String =
            codegen_after(plugin, "index.js", "Object.assign(target, source);")
                .await;

        // The dotted local is rewritten to oxc's `$inject_Object_assign`
        // binding, which the injected import declares.
        assert!(code.contains("$inject_Object_assign"), "{}", code);
        assert!(code.contains("object-assign-shim"), "{}", code);
        assert!(!code.contains("Object.assign"), "{}", code);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_transform_inject_default_import() {
        let plugin: TransformPlugin =
            TransformPlugin::with_options(TransformOptions {
                inject: Some(InjectOptions {
                    entries: vec![InjectEntry {
                        source: String::from("buffer"),
                        specifier: InjectSpecifier::Default {
                            local: String::from("Buffer"),
                        },
                    }],
                }),
                ..TransformOptions::default()
            });

        let code: String =
            codegen_after(plugin, "index.js", "const chunk = new Buffer(8);")
                .await;

        assert!(code.contains("import"), "{}", code);
        assert!(code.contains("buffer"), "{}", code);
        assert!(code.contains("Buffer"), "{}", code);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_transform_inject_skips_when_unreferenced() {
        let plugin: TransformPlugin =
            TransformPlugin::with_options(TransformOptions {
                inject: Some(InjectOptions {
                    entries: vec![InjectEntry {
                        source: String::from("jquery"),
                        specifier: InjectSpecifier::Named {
                            imported: None,
                            local: String::from("$"),
                        },
                    }],
                }),
                ..TransformOptions::default()
            });

        let code: String = codegen_after(plugin, "index.js", "main();").await;

        assert!(!code.contains("import"), "{}", code);
        assert!(!code.contains("jquery"), "{}", code);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_transform_define_identifier() {
        let plugin: TransformPlugin =
            TransformPlugin::with_options(TransformOptions {
                define: Some(DefineOptions {
                    entries: vec![(
                        String::from("__DEV__"),
                        String::from("false"),
                    )],
                }),
                ..TransformOptions::default()
            });

        let code: String =
            codegen_after(plugin, "index.js", "if (__DEV__) { log(); }").await;

        assert!(code.contains("false"), "{}", code);
        assert!(!code.contains("__DEV__"), "{}", code);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_transform_define_chain() {
        let plugin: TransformPlugin =
            TransformPlugin::with_options(TransformOptions {
                define: Some(DefineOptions {
                    entries: vec![(
                        String::from("process.env.NODE_ENV"),
                        String::from("\"production\""),
                    )],
                }),
                ..TransformOptions::default()
            });

        let code: String = codegen_after(
            plugin,
            "index.js",
            "if (process.env.NODE_ENV === \"production\") { ready(); }",
        )
        .await;

        assert!(code.contains("production"), "{}", code);
        assert!(!code.contains("process"), "{}", code);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_transform_define_typeof() {
        let plugin: TransformPlugin =
            TransformPlugin::with_options(TransformOptions {
                define: Some(DefineOptions {
                    entries: vec![(
                        String::from("typeof window"),
                        String::from("\"object\""),
                    )],
                }),
                ..TransformOptions::default()
            });

        let code: String =
            codegen_after(plugin, "index.js", "const kind = typeof window;")
                .await;

        assert!(code.contains("object"), "{}", code);
        assert!(!code.contains("window"), "{}", code);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_transform_define_local_binding_not_replaced() {
        let plugin: TransformPlugin =
            TransformPlugin::with_options(TransformOptions {
                define: Some(DefineOptions {
                    entries: vec![(
                        String::from("__DEV__"),
                        String::from("false"),
                    )],
                }),
                ..TransformOptions::default()
            });

        let code: String = codegen_after(
            plugin,
            "index.js",
            "function main() { const __DEV__ = true; return __DEV__; }",
        )
        .await;

        assert!(
            !code.contains("false"),
            "local shadowing must not be replaced: {}",
            code
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_transform_define_and_inject_combined() {
        let plugin: TransformPlugin =
            TransformPlugin::with_options(TransformOptions {
                inject: Some(InjectOptions {
                    entries: vec![InjectEntry {
                        source: String::from("buffer"),
                        specifier: InjectSpecifier::Default {
                            local: String::from("Buffer"),
                        },
                    }],
                }),
                define: Some(DefineOptions {
                    entries: vec![(
                        String::from("__DEV__"),
                        String::from("false"),
                    )],
                }),
                ..TransformOptions::default()
            });

        let code: String = codegen_after(
            plugin,
            "index.js",
            "if (__DEV__) { new Buffer(8); }",
        )
        .await;

        assert!(code.contains("false"), "{}", code);
        assert!(!code.contains("__DEV__"), "{}", code);
        assert!(code.contains("import"), "{}", code);
        assert!(code.contains("buffer"), "{}", code);
        assert!(code.contains("Buffer"), "{}", code);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_transform_invalid_define_config_errors_before_mutation() {
        let plugin: TransformPlugin =
            TransformPlugin::with_options(TransformOptions {
                define: Some(DefineOptions {
                    entries: vec![(String::from("bad key"), String::from("1"))],
                }),
                ..TransformOptions::default()
            });

        let allocator: Allocator = Allocator::default();

        let mut program: Program<'_> = parse(&allocator, "index.js", "main();");

        let result: TransformReturn =
            run_hook(&plugin, &allocator, "index.js", &mut program).await;

        let error: anyhow::Error =
            result.expect_err("invalid define config must error");

        assert!(error.to_string().contains("bad key"), "{}", error);

        let code: String =
            telarel_common::codegen(telarel_common::CodegenOptions {
                file: "index.js",
                program: &program,
            })
            .code;

        assert!(
            code.contains("main"),
            "program must not be mutated on config error: {}",
            code
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_transform_default_define_inject_unchanged() {
        let plugin: TransformPlugin = TransformPlugin::new();

        let code: String = codegen_after(
            plugin,
            "index.js",
            "if (__DEV__) { process.env.NODE_ENV; }",
        )
        .await;

        assert!(code.contains("__DEV__"), "{}", code);
        assert!(code.contains("process"), "{}", code);
    }
}
