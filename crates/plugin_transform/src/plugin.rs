use std::borrow::Cow;
use std::path::Path;
use std::sync::OnceLock;

use oxc::semantic::SemanticBuilder;
use oxc::transformer::{TransformOptions as OxcTransformOptions, Transformer};
use oxc_transformer_plugins::{InjectGlobalVariables, ReplaceGlobalDefines};

use telarel_common::{Ast, CodegenOptions, HookUsage, codegen};
use telarel_plugin::{Plugin, PluginContext, TransformArgs, TransformOutput};

use crate::options::TransformOptions;
use crate::options::helper_loader::ResolvedHelpers;

/// The builtin transform plugin name.
pub const NAME: &str = "builtin:transform";

/// The resolved, cached plugin state.
#[derive(Debug, Clone)]
struct Resolved {
    /// The oxc transform options for the main transformer pass.
    oxc: OxcTransformOptions,
    /// The resolved helper-loader state for the inline helpers pass.
    helpers: ResolvedHelpers,
    /// The resolved `define` config, `None` when not configured.
    define: Option<oxc_transformer_plugins::ReplaceGlobalDefinesConfig>,
    /// The resolved `inject` config, `None` when not configured.
    inject: Option<oxc_transformer_plugins::InjectGlobalVariablesConfig>,
}

fn render_diagnostics(
    diagnostics: &oxc::diagnostics::Diagnostics
) -> anyhow::Error {
    let rendered: String = diagnostics
        .errors()
        .map(oxc::diagnostics::OxcDiagnostic::render)
        .collect::<Vec<String>>()
        .join("\n");

    anyhow::anyhow!("{rendered}")
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

    async fn transform<'a>(
        &'a self,
        ctx: &'a PluginContext<'_>,
        args: TransformArgs<'a>,
    ) -> telarel_plugin::TransformReturn {
        // The cached resolution is a pure function of the options; the
        // per-call `cwd` from the plugin context overlays on a clone so
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
                        return Err(render_diagnostics(&diagnostics));
                    },
                };

                Ok(Resolved {
                    oxc: self.options.resolve(),
                    helpers: self.options.resolve_helpers(),
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

        // The args AST is read-only; clone it once into a fresh owned AST
        // and run every pass over the clone. The clone is discarded when
        // the passes report no change (`Ok(None)`), which only wastes arena
        // memory.
        let mut ast: Ast = args.ast.clone();

        // Change detection: the main oxc transformer exposes no changed
        // flag, so its effect is probed exactly by printing the program
        // before and after the pass; the inject and define passes report
        // their own `changed`; the helpers pass reports whether it edited
        // the program.
        let before: String = codegen(CodegenOptions {
            file: ctx.module.file,
            program: args.ast.program(),
        })
        .code;

        let mut changed: bool = false;

        ast.with_mut(|allocator, program| -> anyhow::Result<()> {
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
                || codegen(CodegenOptions {
                    file: ctx.module.file,
                    program,
                })
                .code
                    != before;

            // Replace oxc's runtime helper imports with the vendored bodies
            // before inject/define run.
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

#[cfg(test)]
mod tests {
    use oxc::transformer::CompilerAssumptions;
    use oxc::transformer::DecoratorOptions;
    use oxc::transformer::PluginsOptions;

    use telarel_common::{
        Ast, CompileContext, Language, ParseOwnedOptions, parse_owned,
    };
    use telarel_plugin::{ModuleInfo, Plugin, PluginContext, TransformOutput};

    use crate::helpers::sources::{DEPENDENCY_HELPERS, HELPERS, HelperSource};
    use crate::options::TransformOptions;
    use crate::options::define::DefineOptions;
    use crate::options::helper_loader::{
        DEFAULT_HELPER_MODULE_NAME, HelperLoaderMode, HelperLoaderOptions,
    };
    use crate::options::inject::{InjectEntry, InjectOptions, InjectSpecifier};
    use crate::options::jsx::{JsxOptions, JsxRuntime};
    use crate::options::target::TransformTarget;

    use super::*;

    const CWD: &str = "";

    const FILE_TS: &str = "index.ts";

    const FILE_TSX: &str = "index.tsx";

    const SOURCE_TS: &str = "const value: number = 1;";

    const SOURCE_JSX: &str = "const element = <div className=\"x\">hi</div>;";

    const SOURCE_REGEX: &str = "const r = /(/;";

    fn parse(
        file: &str,
        code: &str,
    ) -> Ast {
        let ctx: CompileContext<'_> = CompileContext::new(CWD, file, code);

        parse_owned(ParseOwnedOptions {
            context: &ctx,
            file,
            code,
            language: None,
            source_type: None,
        })
        .expect("valid source parses")
    }

    fn make_context<'a>(file: &'a str) -> PluginContext<'a> {
        let module: ModuleInfo<'a> = ModuleInfo {
            file,
            code: "",
            language: Language::JS,
            source_type: telarel_common::SourceType::Module,
        };

        PluginContext::new(CWD, module)
    }

    async fn run_hook(
        plugin: &TransformPlugin,
        file: &str,
        ast: &Ast,
    ) -> anyhow::Result<Option<TransformOutput>> {
        let ctx: PluginContext<'_> = make_context(file);

        let args: TransformArgs<'_> = TransformArgs { ast };

        plugin.transform(&ctx, args).await
    }

    async fn codegen_after(
        plugin: TransformPlugin,
        file: &str,
        code: &str,
    ) -> String {
        let ast: Ast = parse(file, code);

        // Mirror the driver semantics: `Some` carries the transformed
        // program; `None` means the read-only input stands.
        match run_hook(&plugin, file, &ast).await.expect("transform succeeds") {
            | Some(output) => {
                telarel_common::codegen(telarel_common::CodegenOptions {
                    file,
                    program: output.ast.program(),
                })
                .code
            },
            | None => {
                telarel_common::codegen(telarel_common::CodegenOptions {
                    file,
                    program: ast.program(),
                })
                .code
            },
        }
    }

    fn codegen_output(
        ast: &Ast,
        output: Option<TransformOutput>,
        file: &str,
    ) -> String {
        match &output {
            | Some(output) => {
                telarel_common::codegen(telarel_common::CodegenOptions {
                    file,
                    program: output.ast.program(),
                })
                .code
            },
            | None => {
                telarel_common::codegen(telarel_common::CodegenOptions {
                    file,
                    program: ast.program(),
                })
                .code
            },
        }
    }

    fn inline_helper_plugin() -> TransformPlugin {
        TransformPlugin::with_options(TransformOptions {
            targets: vec![TransformTarget::Es2015],
            helper_loader: Some(HelperLoaderOptions {
                mode: Some(HelperLoaderMode::Inline),
                module_name: None,
            }),
            ..TransformOptions::default()
        })
    }

    /// One source snippet that makes the transform plugin emit a specific
    /// runtime helper, plus the options needed to reach it.
    #[derive(Clone, Copy)]
    struct Trigger {
        helper: &'static str,
        file: &'static str,
        source: &'static str,
        targets: &'static [TransformTarget],
        private_loose: bool,
        legacy_decorators: bool,
        decorator_metadata: bool,
        tagged_template: bool,
        // Proves a dependency-only helper arrived transitively, by a
        // rename-stable marker in the dependent's inlined body.
        dependency_marker: Option<&'static str>,
    }

    const BASE: Trigger = Trigger {
        helper: "",
        file: "index.mjs",
        source: "",
        targets: &[],
        private_loose: false,
        legacy_decorators: false,
        decorator_metadata: false,
        tagged_template: false,
        dependency_marker: None,
    };

    const ES2015: &[TransformTarget] = &[TransformTarget::Es2015];

    const ES2017: &[TransformTarget] = &[TransformTarget::Es2017];

    fn trigger_options(
        trigger: &Trigger,
        mode: HelperLoaderMode,
    ) -> TransformOptions {
        TransformOptions {
            targets: trigger.targets.to_vec(),
            helper_loader: Some(HelperLoaderOptions {
                mode: Some(mode),
                module_name: None,
            }),
            oxc: Some(OxcTransformOptions {
                assumptions: CompilerAssumptions {
                    private_fields_as_properties: trigger.private_loose,
                    ..CompilerAssumptions::default()
                },
                decorator: DecoratorOptions {
                    legacy: trigger.legacy_decorators,
                    emit_decorator_metadata: trigger.decorator_metadata,
                    ..DecoratorOptions::default()
                },
                plugins: PluginsOptions {
                    tagged_template_transform: trigger.tagged_template,
                    ..PluginsOptions::default()
                },
                ..OxcTransformOptions::default()
            }),
            ..TransformOptions::default()
        }
    }

    /// Every vendored helper, mapped to a snippet that makes the transform
    /// plugin emit it. The dependency-only helpers (`OverloadYield`, `get`,
    /// `set`, `typeof`, ...) are never loaded directly by oxc; they are pulled
    /// in transitively, so their entry reuses the dependent snippet and a
    /// rename-stable marker proves they were inlined.
    const TRIGGERS: &[Trigger] = &[
        Trigger {
            helper: "awaitAsyncGenerator",
            source: "async function* g() { await x; }",
            targets: ES2017,
            ..BASE
        },
        Trigger {
            helper: "asyncGeneratorDelegate",
            source: "async function* g() { yield* x; }",
            targets: ES2017,
            ..BASE
        },
        Trigger {
            helper: "asyncIterator",
            source: "async function f() { for await (const x of y) {} }",
            targets: ES2015,
            ..BASE
        },
        Trigger {
            helper: "asyncToGenerator",
            source: "async function f() { await x; }",
            targets: ES2015,
            ..BASE
        },
        Trigger {
            helper: "objectSpread2",
            source: "const o = { ...a, ...b };",
            targets: ES2015,
            ..BASE
        },
        Trigger {
            helper: "wrapAsyncGenerator",
            source: "async function* g() { yield 1; }",
            targets: ES2017,
            ..BASE
        },
        Trigger {
            helper: "extends",
            source: "const { ...r } = obj;",
            targets: ES2015,
            ..BASE
        },
        Trigger {
            helper: "objectDestructuringEmpty",
            source: "const { ...r } = obj;",
            targets: ES2015,
            ..BASE
        },
        Trigger {
            helper: "objectWithoutProperties",
            source: "const { a, ...r } = obj;",
            targets: ES2015,
            ..BASE
        },
        Trigger {
            helper: "toPropertyKey",
            source: "const { [k]: v, ...r } = obj;",
            targets: ES2015,
            ..BASE
        },
        Trigger {
            helper: "defineProperty",
            source: "class C { x = 1; }",
            targets: ES2015,
            ..BASE
        },
        Trigger {
            helper: "classPrivateFieldInitSpec",
            source: "class C { #x = 1; m() { return this.#x; } }",
            targets: ES2015,
            ..BASE
        },
        Trigger {
            helper: "classPrivateMethodInitSpec",
            source: "class C { #m() {} c() { this.#m(); } }",
            targets: ES2015,
            ..BASE
        },
        Trigger {
            helper: "classPrivateFieldGet2",
            source: "class C { #x = 1; m() { return this.#x; } }",
            targets: ES2015,
            ..BASE
        },
        Trigger {
            helper: "classPrivateFieldSet2",
            source: "class C { #x = 1; m() { this.#x = 2; } }",
            targets: ES2015,
            ..BASE
        },
        Trigger {
            helper: "assertClassBrand",
            source: "class C { #m() {} c() { this.#m(); } }",
            targets: ES2015,
            ..BASE
        },
        Trigger {
            helper: "toSetter",
            source: "class C { set #x(v) {} m() { this.#x = 1; } }",
            targets: ES2015,
            ..BASE
        },
        Trigger {
            helper: "classPrivateFieldLooseKey",
            source: "class C { #x = 1; m() { return this.#x; } }",
            targets: ES2015,
            private_loose: true,
            ..BASE
        },
        Trigger {
            helper: "classPrivateFieldLooseBase",
            source: "class C { #x = 1; m() { return this.#x; } }",
            targets: ES2015,
            private_loose: true,
            ..BASE
        },
        Trigger {
            helper: "superPropGet",
            source: "class C extends B { static { const z = super.x; } }",
            targets: ES2015,
            ..BASE
        },
        Trigger {
            helper: "superPropSet",
            source: "class C extends B { static { super.x = 1; } }",
            targets: ES2015,
            ..BASE
        },
        Trigger {
            helper: "readOnlyError",
            source: "class C { get #x() { return 1; } m() { this.#x = 2; } }",
            targets: ES2015,
            ..BASE
        },
        Trigger {
            helper: "writeOnlyError",
            source: "class C { set #x(v) {} m() { return this.#x; } }",
            targets: ES2015,
            ..BASE
        },
        Trigger {
            helper: "checkInRHS",
            source: "class C { #x; has(o) { return #x in o; } }",
            targets: ES2015,
            ..BASE
        },
        Trigger {
            helper: "decorate",
            file: "index.ts",
            source: "@dec class C {}",
            legacy_decorators: true,
            ..BASE
        },
        Trigger {
            helper: "decorateParam",
            file: "index.ts",
            source: "class C { m(@dec x: any) {} }",
            legacy_decorators: true,
            ..BASE
        },
        Trigger {
            helper: "decorateMetadata",
            file: "index.ts",
            source: "class C { @dec m() {} }",
            legacy_decorators: true,
            decorator_metadata: true,
            ..BASE
        },
        Trigger {
            helper: "usingCtx",
            source: "using x = y;",
            targets: ES2015,
            ..BASE
        },
        Trigger {
            helper: "taggedTemplateLiteral",
            source: "const t = tag`</script>`;",
            tagged_template: true,
            ..BASE
        },
        Trigger {
            helper: "OverloadYield",
            source: "async function* g() { yield 1; }",
            targets: ES2017,
            dependency_marker: Some("this.v = e, this.k = d"),
            ..BASE
        },
        Trigger {
            helper: "checkPrivateRedeclaration",
            source: "class C { #x = 1; m() { return this.#x; } }",
            targets: ES2015,
            dependency_marker: Some(
                "Cannot initialize the same private elements twice on an object",
            ),
            ..BASE
        },
        Trigger {
            helper: "get",
            source: "class C extends B { static { const z = super.x; } }",
            targets: ES2015,
            dependency_marker: Some("Reflect.get.bind"),
            ..BASE
        },
        Trigger {
            helper: "getPrototypeOf",
            source: "class C extends B { static { const z = super.x; } }",
            targets: ES2015,
            dependency_marker: Some(
                "Object.setPrototypeOf ? Object.getPrototypeOf.bind()",
            ),
            ..BASE
        },
        Trigger {
            helper: "objectWithoutPropertiesLoose",
            source: "const { a, ...r } = obj;",
            targets: ES2015,
            dependency_marker: Some(".includes(n)) continue"),
            ..BASE
        },
        Trigger {
            helper: "set",
            source: "class C extends B { static { super.x = 1; } }",
            targets: ES2015,
            dependency_marker: Some("failed to set property"),
            ..BASE
        },
        Trigger {
            helper: "superPropBase",
            source: "class C extends B { static { const z = super.x; } }",
            targets: ES2015,
            dependency_marker: Some(
                "!{}.hasOwnProperty.call(t, o) && null !==",
            ),
            ..BASE
        },
        Trigger {
            helper: "toPrimitive",
            source: "const o = { ...a, ...b };",
            targets: ES2015,
            dependency_marker: Some(
                "@@toPrimitive must return a primitive value.",
            ),
            ..BASE
        },
        Trigger {
            helper: "typeof",
            source: "const o = { ...a, ...b };",
            targets: ES2015,
            dependency_marker: Some("@babel/helpers - typeof"),
            ..BASE
        },
    ];

    /// The local binding oxc gave the default import/require of `specifier`.
    ///
    /// Matches both emitted shapes: `import <local> from "<specifier>";` and
    /// `var <local> = require("<specifier>");`.
    fn runtime_import_local(
        code: &str,
        specifier: &str,
    ) -> Option<String> {
        for line in code.lines() {
            let line: &str = line.trim();

            if let Some(import) = line.strip_prefix("import ") {
                let Some(local) = import.split(" from ").next() else {
                    continue;
                };

                if line.contains(&format!("\"{specifier}\"")) {
                    return Some(local.trim().to_string());
                }
            }

            if let Some(require) = line.strip_prefix("var ") {
                if !line.contains(&format!("require(\"{specifier}\")")) {
                    continue;
                }

                if let Some(local) = require.split(" = ").next() {
                    return Some(local.trim().to_string());
                }
            }
        }

        None
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
    async fn test_transform_changed_returns_some_with_program() {
        // A lowering that rewrites the tree must return `Some` carrying the
        // transformed program, and the carried program must reflect the edit.
        let plugin: TransformPlugin = inline_helper_plugin();

        let ast: Ast =
            parse("index.js", "async function main() { await g(); }");

        let output: Option<TransformOutput> =
            run_hook(&plugin, "index.js", &ast)
                .await
                .expect("transform succeeds");

        let output: TransformOutput =
            output.expect("changed program must return Some");

        // The transformed program is a distinct allocation; the input
        // program stays untouched.
        let input_code: String =
            telarel_common::codegen(telarel_common::CodegenOptions {
                file: "index.js",
                program: ast.program(),
            })
            .code;

        assert!(input_code.contains("async function"), "{input_code}");

        let code: String =
            telarel_common::codegen(telarel_common::CodegenOptions {
                file: "index.js",
                program: output.ast.program(),
            })
            .code;

        assert!(!code.contains("async function"), "{code}");
        assert!(code.contains("asyncToGenerator"), "{code}");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_transform_unchanged_returns_none() {
        // EsNext targets leave the already-clean tree untouched; the builtin
        // plugin must report the no-op as `None`.
        let plugin: TransformPlugin =
            TransformPlugin::with_options(TransformOptions {
                targets: vec![TransformTarget::EsNext],
                helper_loader: Some(HelperLoaderOptions {
                    mode: Some(HelperLoaderMode::Runtime),
                    module_name: None,
                }),
                ..TransformOptions::default()
            });

        let ast: Ast = parse("index.js", "main();");

        let output: Option<TransformOutput> =
            run_hook(&plugin, "index.js", &ast)
                .await
                .expect("transform succeeds");

        assert!(output.is_none(), "unchanged program must return None");

        let code: String =
            telarel_common::codegen(telarel_common::CodegenOptions {
                file: "index.js",
                program: ast.program(),
            })
            .code;

        assert!(code.contains("main"), "{code}");
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
    async fn test_transform_bare_default_inlines_helpers() {
        // The unconfigured helper state defaults to inline mode; the bare
        // default options must lower, emit runtime imports, and then inline
        // the helper bodies in the post-transform pass.
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
            !code.contains("@oxc-project/runtime"),
            "runtime import must be removed: {code}"
        );
        assert!(code.contains("function _asyncToGenerator("), "{code}");
        assert!(code.contains("_asyncToGenerator(function* ()"), "{code}");
        assert!(!code.contains("async function"), "{}", code);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_transform_inline_helper_mode_does_not_reach_oxc_inline() {
        // oxc's `Inline` mode panics at helper load time; telarel maps it to
        // oxc `Runtime` until the inline helpers pass replaces it.
        let plugin: TransformPlugin =
            TransformPlugin::with_options(TransformOptions {
                targets: vec![TransformTarget::Es2015],
                helper_loader: Some(HelperLoaderOptions {
                    mode: Some(HelperLoaderMode::Inline),
                    module_name: None,
                }),
                ..TransformOptions::default()
            });

        let ast: Ast =
            parse("index.js", "async function main() { await g(); }");

        let output: Option<TransformOutput> =
            run_hook(&plugin, "index.js", &ast)
                .await
                .expect("transform succeeds");

        let resolved = plugin
            .resolved
            .get()
            .expect("cached resolution")
            .as_ref()
            .expect("resolution succeeded");

        assert!(resolved.helpers.inline());

        let output: TransformOutput =
            output.expect("the inline helper pass must change the program");

        let code: String =
            telarel_common::codegen(telarel_common::CodegenOptions {
                file: "index.js",
                program: output.ast.program(),
            })
            .code;

        assert!(!code.contains("@oxc-project/runtime"), "{}", code);
        assert!(
            code.contains("function _asyncToGenerator(")
                && code.contains("_asyncToGenerator(function* ()"),
            "{}",
            code
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_transform_inline_helpers_user_default_import_from_helper_path()
     {
        // A user default import from a runtime helpers path shares the helper
        // source with the oxc-generated uid; both bindings must end up on the
        // single inlined function.
        let plugin: TransformPlugin = inline_helper_plugin();

        let code: String = codegen_after(
            plugin,
            "index.mjs",
            "import _asyncToGenerator from \"@oxc-project/runtime/helpers/asyncToGenerator\";\nasync function main() { await g(); }",
        )
        .await;

        assert!(
            !code.contains("@oxc-project/runtime"),
            "runtime imports must be removed: {code}"
        );
        assert!(code.contains("function _asyncToGenerator("), "{code}");
        assert!(code.contains("_asyncToGenerator(function* ()"), "{code}");
        assert_eq!(
            code.matches("function _asyncToGenerator(").count(),
            1,
            "helper must be inlined exactly once: {code}"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_transform_inline_helpers_user_named_import_from_helper_path_untouched()
     {
        // A named (non-default) import from a helpers path never matches the
        // oxc-emitted shape; it is user code and must be left untouched.
        let plugin: TransformPlugin = inline_helper_plugin();

        let code: String = codegen_after(
            plugin,
            "index.mjs",
            "import { other } from \"@oxc-project/runtime/helpers/asyncToGenerator\";\nasync function main() { await g(); }\nexport { other };",
        )
        .await;

        assert!(
            code.contains("import { other } from \"@oxc-project/runtime/helpers/asyncToGenerator\""),
            "user import must be untouched: {code}"
        );
        assert!(
            code.contains("function _asyncToGenerator("),
            "the oxc-generated helper import is still inlined: {code}"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_transform_invalid_regexp_errors() {
        let plugin: TransformPlugin =
            TransformPlugin::with_options(TransformOptions {
                targets: vec![TransformTarget::Es2015],
                ..TransformOptions::default()
            });

        let ast: Ast = parse("index.js", SOURCE_REGEX);

        let result: anyhow::Result<Option<TransformOutput>> =
            run_hook(&plugin, "index.js", &ast).await;

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

        let ast: Ast = parse("index.js", "const value = await promise;");

        let result: anyhow::Result<Option<TransformOutput>> =
            run_hook(&plugin, "index.js", &ast).await;

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
        // A non-empty plugin context `cwd` must overlay onto the resolved
        // oxc options; the plugin-level cache stays cwd-free.
        let plugin: TransformPlugin =
            TransformPlugin::with_options(TransformOptions {
                targets: vec![TransformTarget::Es2015],
                ..TransformOptions::default()
            });

        let ast: Ast = parse("index.js", "async function main() {}");

        let module: ModuleInfo<'_> = ModuleInfo {
            file: "index.js",
            code: "",
            language: Language::JS,
            source_type: telarel_common::SourceType::Module,
        };

        let ctx: PluginContext<'_> = PluginContext::new("/repo", module);

        let args: TransformArgs<'_> = TransformArgs { ast: &ast };

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

        let first: Ast = parse("index.js", "async function main() {}");

        let first_output: Option<TransformOutput> =
            run_hook(&plugin, "index.js", &first)
                .await
                .expect("first transform succeeds");

        let second: Ast = parse("index.js", "async function other() {}");

        let second_output: Option<TransformOutput> =
            run_hook(&plugin, "index.js", &second)
                .await
                .expect("second transform succeeds");

        let first_code: String =
            codegen_output(&first, first_output, "index.js");

        let second_code: String =
            codegen_output(&second, second_output, "index.js");

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

        let ast: Ast = parse("index.js", "main();");

        let result: anyhow::Result<Option<TransformOutput>> =
            run_hook(&plugin, "index.js", &ast).await;

        let error: anyhow::Error =
            result.expect_err("invalid define config must error");

        assert!(error.to_string().contains("bad key"), "{}", error);

        let code: String =
            telarel_common::codegen(telarel_common::CodegenOptions {
                file: "index.js",
                program: ast.program(),
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

    #[tokio::test(flavor = "current_thread")]
    async fn test_transform_inline_helpers_esm_import_form() {
        let plugin: TransformPlugin = inline_helper_plugin();

        let code: String = codegen_after(
            plugin,
            "index.mjs",
            "async function main() { await g(); }",
        )
        .await;

        // The runtime import is replaced by the inlined helper body; the
        // helper-internal `asyncGeneratorStep` is inlined under a fresh uid.
        assert!(
            !code.contains("@oxc-project/runtime"),
            "runtime import must be removed: {code}"
        );
        assert!(
            !code.contains("import "),
            "helper import must be removed: {code}"
        );
        assert!(code.contains("function _asyncToGenerator("), "{code}");
        assert!(code.contains("function _asyncGeneratorStep("), "{code}");
        assert!(code.contains("_asyncToGenerator(function* ()"), "{code}");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_transform_inline_helpers_require_form() {
        // A `.cjs` file parses as a script, so oxc emits the require form.
        let plugin: TransformPlugin = inline_helper_plugin();

        let code: String = codegen_after(
            plugin,
            "index.cjs",
            "async function main() { await g(); }",
        )
        .await;

        assert!(
            !code.contains("require("),
            "helper require must be removed: {code}"
        );
        assert!(
            !code.contains("@oxc-project/runtime"),
            "runtime specifier must be removed: {code}"
        );
        assert!(code.contains("function _asyncToGenerator("), "{code}");
        assert!(code.contains("_asyncToGenerator(function* ()"), "{code}");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_transform_inline_helpers_transitive_dependencies() {
        let plugin: TransformPlugin = inline_helper_plugin();

        let code: String =
            codegen_after(plugin, "index.mjs", "const out = { ...a, ...b };")
                .await;

        assert!(
            !code.contains("@oxc-project/runtime"),
            "runtime imports must be removed: {code}"
        );

        // The full dependency closure of `objectSpread2` is inlined once each,
        // dependencies before dependents: objectSpread2 -> defineProperty ->
        // toPropertyKey -> { typeof, toPrimitive } -> typeof.
        for name in [
            "_objectSpread",
            "_defineProperty",
            "_toPropertyKey",
            "_toPrimitive",
            "_typeof",
            "_ownKeys",
        ] {
            assert!(code.contains(&format!("function {name}(")), "{code}");
        }

        assert!(code.contains("const out = _objectSpread("), "{code}");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_transform_inline_helpers_multiple_helpers() {
        let plugin: TransformPlugin = inline_helper_plugin();

        let code: String = codegen_after(
            plugin,
            "index.mjs",
            "async function main() { await g(); const out = { ...a, ...b }; }",
        )
        .await;

        assert!(
            !code.contains("@oxc-project/runtime"),
            "runtime imports must be removed: {code}"
        );
        assert!(code.contains("function _asyncToGenerator("), "{code}");
        assert!(code.contains("function _objectSpread("), "{code}");
        assert!(code.contains("_asyncToGenerator(function* ()"), "{code}");
        assert!(code.contains("_objectSpread("), "{code}");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_transform_inline_helpers_name_collision_with_user_code() {
        let plugin: TransformPlugin = inline_helper_plugin();

        // The user's own `_typeof` collides with the `typeof` dependency
        // helper of the objectSpread2 closure; the inlined dependency must be
        // renamed to a fresh uid and the user binding must be untouched.
        let code: String = codegen_after(
            plugin,
            "index.mjs",
            "function _typeof() { return 1; }\nconst k = { ...obj };",
        )
        .await;

        assert!(code.contains("function _typeof() {\n\treturn 1;"), "{code}");
        assert!(
            code.contains("function _typeof2(o)"),
            "inlined typeof must be renamed: {code}"
        );
        assert!(
            code.contains("_typeof2(t)"),
            "references must be renamed to the fresh uid: {code}"
        );
        assert!(
            !code.contains("@oxc-project/runtime"),
            "runtime imports must be removed: {code}"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_transform_inline_helpers_noop_when_mode_is_explicit_runtime()
    {
        // With an explicit runtime mode the oxc runtime import must be
        // emitted unchanged; the inline pass must not touch the program.
        let plugin: TransformPlugin =
            TransformPlugin::with_options(TransformOptions {
                targets: vec![TransformTarget::Es2015],
                helper_loader: Some(HelperLoaderOptions {
                    mode: Some(HelperLoaderMode::Runtime),
                    module_name: None,
                }),
                ..TransformOptions::default()
            });

        let code: String = codegen_after(
            plugin,
            "index.mjs",
            "async function main() { await g(); }",
        )
        .await;

        assert!(
            code.contains("@oxc-project/runtime/helpers/asyncToGenerator"),
            "{code}"
        );
        assert!(
            !code.contains("function _asyncToGenerator("),
            "helper body must not be inlined: {code}"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_transform_inline_helpers_mixed_shape_helper() {
        // `classPrivateFieldLooseKey` is a `Mixed`-shape helper: a top-level
        // `var id = 0` counter plus the function declaration. The counter
        // binding is renamed to a fresh uid, and the statement order is kept.
        let plugin: TransformPlugin =
            TransformPlugin::with_options(TransformOptions {
                helper_loader: Some(HelperLoaderOptions {
                    mode: Some(HelperLoaderMode::Inline),
                    module_name: None,
                }),
                oxc: Some(OxcTransformOptions {
                    env: oxc::transformer::EnvOptions {
                        es2022: oxc::transformer::ES2022Options {
                            class_properties: Some(
                                oxc::transformer::ClassPropertiesOptions {
                                    loose: true,
                                },
                            ),
                            ..oxc::transformer::ES2022Options::default()
                        },
                        ..oxc::transformer::EnvOptions::default()
                    },
                    ..OxcTransformOptions::default()
                }),
                ..TransformOptions::default()
            });

        let code: String = codegen_after(
            plugin,
            "index.mjs",
            "class A { #x = 1; read() { return this.#x; } }",
        )
        .await;

        assert!(
            !code.contains("@oxc-project/runtime"),
            "runtime imports must be removed: {code}"
        );
        assert!(
            code.contains("var _id = 0;"),
            "top-level var statement must be inlined: {code}"
        );
        assert!(
            code.contains("function _classPrivateFieldLooseKey("),
            "{code}"
        );
        assert!(
            code.contains("\"__private_\" + _id++ + \"_\""),
            "counter references must be renamed: {code}"
        );
        assert!(
            code.contains("function _classPrivateFieldLooseBase("),
            "{code}"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_transform_inline_helpers_keep_user_code_and_directives() {
        // The inlined statements land at the top of the body (directives live
        // in `Program::directives`), and user code other than the helper
        // imports is untouched.
        let plugin: TransformPlugin = inline_helper_plugin();

        let code: String = codegen_after(
            plugin,
            "index.mjs",
            "\"use strict\";\nconst marker = 1;\nasync function main() { await g(); }",
        )
        .await;

        assert!(code.contains("\"use strict\""), "{code}");
        assert!(code.contains("const marker = 1;"), "{code}");
        assert!(code.contains("function _asyncToGenerator("), "{code}");

        let helper_index: usize =
            code.find("function _asyncToGenerator(").expect("helper present");

        let marker_index: usize =
            code.find("const marker").expect("user code present");

        assert!(
            helper_index < marker_index,
            "inlined helpers must precede user statements: {code}"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_every_helper_is_triggered_and_inlined() {
        let module_name: &str = DEFAULT_HELPER_MODULE_NAME;

        for trigger in TRIGGERS {
            let runtime_plugin: TransformPlugin = TransformPlugin::with_options(
                trigger_options(trigger, HelperLoaderMode::Runtime),
            );

            let inline_plugin: TransformPlugin = TransformPlugin::with_options(
                trigger_options(trigger, HelperLoaderMode::Inline),
            );

            let runtime_code: String =
                codegen_after(runtime_plugin, trigger.file, trigger.source)
                    .await;

            let inline_code: String =
                codegen_after(inline_plugin, trigger.file, trigger.source)
                    .await;

            let specifier: String =
                format!("{module_name}/helpers/{}", trigger.helper);

            match trigger.dependency_marker {
                | Some(marker) => {
                    // Dependency-only helper: oxc never loads it directly, so
                    // prove it arrived transitively by its inlined body.
                    assert!(
                        !runtime_code.contains(&specifier),
                        "dependency-only `{}` must not be loaded directly: {runtime_code}",
                        trigger.helper
                    );
                    assert!(
                        inline_code.contains(marker),
                        "dependency `{}` must be inlined ({marker}): {inline_code}",
                        trigger.helper
                    );
                },
                | None => {
                    // Direct helper: the snippet must make the plugin emit
                    // the runtime import for this helper...
                    assert!(
                        runtime_code.contains(&specifier),
                        "snippet must trigger helper `{}` ({specifier}): {runtime_code}",
                        trigger.helper
                    );

                    // ...and the inline pass must bind the helper's body under
                    // the very local name oxc chose for that import, proving
                    // the emitted import was replaced by the inlined function.
                    let local: String =
                        runtime_import_local(&runtime_code, &specifier)
                            .unwrap_or_else(|| {
                                panic!(
                                    "runtime import for `{}` must bind a local: {runtime_code}",
                                    trigger.helper
                                )
                            });

                    assert!(
                        inline_code.contains(&format!("function {local}(")),
                        "helper `{}` ({local}) must be inlined as a function: {inline_code}",
                        trigger.helper
                    );
                },
            }

            // ...and the inline pass must consume every runtime import.
            assert!(
                !inline_code.contains(module_name),
                "helper `{}` runtime import must be inlined: {inline_code}",
                trigger.helper
            );
        }
    }

    #[test]
    fn test_trigger_table_covers_every_vendored_helper() {
        // A newly vendored (or removed) helper must not silently lose its
        // trigger; the table is the source of truth for plugin-level coverage.
        let expected: Vec<&str> = HELPERS
            .iter()
            .chain(DEPENDENCY_HELPERS.iter())
            .map(|helper: &HelperSource| helper.name)
            .collect();

        let covered: Vec<&str> =
            TRIGGERS.iter().map(|trigger: &Trigger| trigger.helper).collect();

        for name in &expected {
            assert!(
                covered.contains(name),
                "helper `{name}` has no trigger entry"
            );
        }

        assert_eq!(
            covered.len(),
            expected.len(),
            "trigger table must cover every helper exactly once"
        );
    }
}
