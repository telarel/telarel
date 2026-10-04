//! # Telarel Core
//!
//! A pipeline orchestrator for the compiler.
//!
//! This crate drives the per-file compile pipeline: it runs plugin hooks,
//! parses the source, applies transforms, and produces code with a map.

mod resolve;

use oxc::allocator::Allocator;
use oxc::ast::ast::Program;
use oxc_sourcemap::SourceMapBuilder;

use telarel_common::{
    CompileContext, CompileError, HookUsage, Language, ParseOptions,
    ParseResult, ResolvedOptions, SourceType, compose_maps, parse,
};
use telarel_plugin::__internal::PluginDriver;
use telarel_plugin::{
    CommonPluginContext, CompileEndArgs, CompileStartArgs, FinalizeArgs,
    OptionsArgs, PluginContext, PrepareArgs, SharedPluginable, TransformArgs,
    options_fixpoint,
};

use crate::resolve::language::infer_language;
use crate::resolve::source_type::resolve_source_type;

pub use telarel_common::CompileOptions;
pub use telarel_common::SourceMap;

/// Strip a trailing `\n`, then a trailing `\r`, so `\n`, `\r\n`, and
/// lone `\r` terminators disappear while interior line endings survive.
///
/// Input is owned, so the terminator is removed in place rather than
/// copying the whole string into a fresh `String`.
fn strip_trailing_newline(mut source: String) -> String {
    let new_len: usize =
        source.trim_end_matches('\n').trim_end_matches('\r').len();

    source.truncate(new_len);

    source
}

/// Build a per-line identity map: each generated line maps to the same
/// source line at column 0.
fn identity_map(
    file: &str,
    code: &str,
) -> SourceMap {
    let mut builder: SourceMapBuilder<'_> = SourceMapBuilder::default();

    builder.set_file(file);

    let source_id: u32 = builder.add_source_and_content(file, code);

    let line_count: u32 =
        u32::try_from(code.lines().count().max(1)).unwrap_or(u32::MAX);

    for line in 0..line_count {
        builder.add_token(line, 0, line, 0, Some(source_id), None);
    }

    builder.into_owned_sourcemap().into_inner()
}

/// The pipeline tail after the `finalize` fold.
struct TailOutput {
    code: String,
    map: SourceMap,
}

/// The last-good `(code, map)` state for a stage failure.
struct LastGood {
    code: String,
    map: Option<SourceMap>,
}

/// A stage failure with the last-good state at the point of failure.
struct StageError {
    /// The original, stage-wrapped error (`{stage} hook: {err}`).
    error: CompileError,
    /// The last good `(code, map)` state before the failure.
    last_good: LastGood,
}

/// The post-settle pipeline tail outcome: either the final output or a
/// stage failure carrying the last-good `(code, map)` state.
type TailResult = Result<TailOutput, StageError>;

/// Result of a successful compile.
#[derive(Debug, Clone)]
pub struct CompileOutput {
    /// Generated code.
    pub code: String,
    /// Source map.
    pub map: SourceMap,
}

/// Run the stage tail (R5 steps 4-9): `compile_start`, `prepare`, the gated
/// parse/transform/codegen path, and `finalize`.
///
/// R6 map composition: every map returned by a `prepare` hook is incremental
/// relative to the code that hook received, so the prepare-side chain composes
/// against the ORIGINAL `resolved.code` via [`compose_maps`]; a chain of
/// `finalize` maps composes in the reverse direction (the LAST-served map owns
/// the final-output dst tokens and the earlier maps chain behind it, ending
/// with the codegen/identity map that lands in the original source). A hook
/// that omits its map resets the collection; when NO map was returned the
/// map stays the parse/codegen map referencing the actual parse input.
///
/// On failure, the error carries the last-good state so `compile_end` can be
/// invoked with it; the original stage-wrapped error shape is preserved.
async fn run_stages(
    driver: &PluginDriver,
    plugin_ctx: &PluginContext<'_>,
    resolved_options: &ResolvedOptions,
    ctx: &CompileContext<'_>,
    resolved: &CompileOptions,
) -> TailResult {
    driver
        .compile_start(
            plugin_ctx,
            &CompileStartArgs { options: resolved_options.clone() },
        )
        .await
        .map_err(|error| StageError {
            error: CompileError::from_message(&format!(
                "compile_start hook: {error:#}"
            )),
            last_good: LastGood { code: resolved.code.clone(), map: None },
        })?;

    // R5 step 5: the `prepare` fold over the original source; `Some` replaces
    // the code. Every returned map is collected in fold order (a returned
    // map-less output resets the collection).
    let prepare_fold: telarel_plugin::__internal::PrepareFold = driver
        .prepare(plugin_ctx, &PrepareArgs { code: &resolved.code })
        .await
        .map_err(|error| StageError {
            error: CompileError::from_message(&format!(
                "prepare hook: {error:#}"
            )),
            last_good: LastGood { code: resolved.code.clone(), map: None },
        })?;

    let prepare_output: Option<telarel_plugin::PrepareOutput> =
        prepare_fold.output;

    // The prepare-map composition channel (R6): compose the whole CHAIN of
    // returned maps, not just the last one. Each hook's map is incremental
    // relative to the code that hook RECEIVED (dst space = hook output,
    // src space = hook input), so resolving the parse input (code_n) back
    // to the original `resolved.code` (code_0) applies the maps in REVERSE
    // fold order: the last-served map's dst space is the parse input and
    // resolves first. With one map this reduces to the single-map
    // composition. Omitted maps reset the chain (the fold's rule), so the
    // surviving segment composes against the actual parse input.
    let prepare_maps: Vec<SourceMap> = prepare_fold
        .maps
        .iter()
        .rev()
        .filter_map(|out: &telarel_plugin::PrepareOutput| out.map.clone())
        .collect::<Vec<SourceMap>>();

    let parse_code: String =
        prepare_output.map_or_else(|| resolved.code.clone(), |out| out.code);

    let (code, map): (String, SourceMap) =
        if driver.usage().contains(HookUsage::Transform) {
            // Parse ONCE before the transform chain. Root the resolved source in
            // the allocator so the parsed program borrows from the same
            // allocation.
            let allocator: Allocator = Allocator::default();

            let source: &'_ str = allocator.alloc_str(&parse_code);

            let parse_options: ParseOptions<'_, '_> = ParseOptions {
                context: ctx,
                allocator: &allocator,
                file: &resolved.file,
                code: source,
                language: Some(resolved_options.language),
                source_type: Some(resolved_options.source_type),
            };

            let parse_result: ParseResult<'_> =
                parse(parse_options).map_err(|error| StageError {
                    error,
                    last_good: LastGood {
                        code: parse_code.clone(),
                        // `prepare_maps` is in reverse fold order; the
                        // FIRST element is the LAST-SERVED surviving
                        // map (the map closest to the parse input).
                        map: prepare_maps.first().cloned(),
                    },
                })?;

            let parse_program: &Program<'_> = &parse_result.program;

            let transform_args: TransformArgs<'_> =
                TransformArgs { allocator: &allocator, ast: parse_program };

            let transform_output: Option<telarel_plugin::TransformOutput<'_>> =
                driver.transform(plugin_ctx, &transform_args).await.map_err(
                    |error| StageError {
                        error: CompileError::from_message(&format!(
                            "transform hook: {error:#}"
                        )),
                        last_good: LastGood {
                            code: parse_code.clone(),
                            // Reverse fold order; the first map is the
                            // LAST-SERVED surviving one.
                            map: prepare_maps.first().cloned(),
                        },
                    },
                )?;

            // The driver's `Some` carries the last replacement; `None`
            // means no plugin changed anything, so the parse result is
            // codegen'd directly. Both are rooted in the compile
            // allocator; no copy is needed.
            let final_program: &Program<'_> = match &transform_output {
                | Some(output) => output.ast,
                | None => parse_program,
            };

            let result: telarel_common::CodegenResult<'_> =
                telarel_common::codegen(telarel_common::CodegenOptions {
                    file: &resolved.file,
                    program: final_program,
                });

            let code: String = strip_trailing_newline(result.code);

            let map: SourceMap = result.map.into_owned();

            (code, map)
        } else {
            // No plugin uses `transform`: skip parse and codegen entirely and
            // pass the source through with a per-line identity map. A clone is
            // required here: `ctx` borrows the parse source until the `finalize`
            // hook below, so the strip cannot consume it in place.
            let code: String = strip_trailing_newline(parse_code.clone());

            let map: SourceMap = identity_map(&resolved.file, &parse_code);

            (code, map)
        };

    // R5 step 9: the `finalize` fold over the generated code; `Some` replaces
    // the carried code. Every returned map is collected in fold order (a
    // returned map-less output resets the collection).
    let finalize_fold: telarel_plugin::__internal::FinalizeFold = driver
        .finalize(plugin_ctx, &FinalizeArgs { code: &code })
        .await
        .map_err(|error| {
            // The error-path last good is the UN-composed codegen/pipeline
            // map (the finalize hook's own map never composed); `compile_end`
            // reports last good, composition applies to the OUTPUT map only.
            let last_good: LastGood =
                LastGood { code: code.clone(), map: Some(map.clone()) };

            StageError {
                error: CompileError::from_message(&format!(
                    "finalize hook: {error:#}"
                )),
                last_good,
            }
        })?;

    let finalize_output: Option<telarel_plugin::FinalizeOutput> =
        finalize_fold.output;

    let code: String =
        finalize_output.map_or_else(|| code.clone(), |out| out.code);

    // The finalize-map composition channel (R6): compose the whole CHAIN of
    // returned maps, not just the last one. Each hook's map is incremental
    // relative to the code that hook RECEIVED (dst space = hook output,
    // src space = hook input; the first hook's input is the generated
    // code). `finalize_maps` is in REVERSE fold order, so its FIRST element is
    // the LAST-SERVED surviving map — its dst space is the FINAL output and
    // it must own the output dst tokens. Composing it as the `upstream` and
    // every earlier map (then the pipeline map) as an incremental resolves
    // final-output positions backward to the generated code and on to the
    // original source (final -> code_n -> ... -> generated -> original).
    // Omitted maps reset the chain (the fold's rule), so the surviving
    // segment composes against the actual generated code. With one finalize map
    // this reduces to the shipped single-map direction (finalize map upstream,
    // pipeline map incremental).
    let finalize_maps: Vec<SourceMap> = finalize_fold
        .maps
        .iter()
        .rev()
        .filter_map(|out: &telarel_plugin::FinalizeOutput| out.map.clone())
        .collect::<Vec<SourceMap>>();

    let pipeline_map: SourceMap = compose_maps(
        Some(&resolved.file),
        &map,
        prepare_maps.iter().collect::<Vec<&SourceMap>>().as_slice(),
    );

    let map: SourceMap = match finalize_maps.split_first() {
        | Some((upstream, rest)) => {
            // Earlier finalize maps chain behind the last one in reverse fold
            // order; the pipeline map is the final incremental that lands
            // in the ORIGINAL source (its dst space is the generated code).
            let mut incrementals: Vec<&SourceMap> = rest.iter().collect();

            incrementals.push(&pipeline_map);

            compose_maps(Some(&resolved.file), upstream, &incrementals)
        },
        | None => pipeline_map,
    };

    Ok(TailOutput { code, map })
}

/// Resolve an omitted `cwd` to the runtime's process working directory,
/// falling back to `/` when it cannot be read.
fn resolve_process_cwd() -> String {
    std::env::current_dir()
        .map(|p: std::path::PathBuf| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| String::from("/"))
}

/// Run the per-file pipeline over `options` with `plugins`.
pub async fn compile(
    options: CompileOptions,
    plugins: Vec<SharedPluginable>,
) -> Result<CompileOutput, CompileError> {
    let mut resolved: CompileOptions = options;

    resolved.cwd.get_or_insert_with(resolve_process_cwd);

    // The common context is constructed ONCE per compile: the `options`
    // fixpoint runs against this instance, and the same instance is shared
    // with every later hook through `PluginContext`.
    let common: CommonPluginContext = CommonPluginContext::default();

    let args: OptionsArgs = OptionsArgs { options: resolved.clone(), plugins };

    // The `options` hooks run as a fixpoint BEFORE any driver exists;
    // plugins injected into the args join the settled list.
    let (args, settled_plugins): (OptionsArgs, Vec<SharedPluginable>) =
        options_fixpoint(&common, &args).await.map_err(|error| {
            CompileError::from_message(&format!("options hook: {error:#}"))
        })?;

    let mut resolved: CompileOptions = args.options;

    // A late `options` hook may return a bag that omits `cwd`; re-resolve it
    // so `compile_start` and later hooks observe the process working
    // directory rather than a silent fallback.
    resolved.cwd.get_or_insert_with(resolve_process_cwd);

    // The driver is built from the SETTLED list: its usage aggregate covers
    // plugins injected during the `options` stage, so the parse-skip gate
    // below counts them.
    let driver: PluginDriver = PluginDriver::new(settled_plugins);

    // R5 step 4: fully-resolved options for `compile_start`; the settled
    // name list preserves order and duplicates.
    let language: Language = infer_language(&resolved.file, resolved.language);

    let source_type: SourceType = resolve_source_type(
        &resolved.file,
        resolved.language,
        resolved.source_type,
    );

    let resolved_options: ResolvedOptions = ResolvedOptions {
        cwd: resolved.cwd.clone().unwrap_or_else(|| String::from("/")),
        file: resolved.file.clone(),
        code: resolved.code.clone(),
        language,
        source_type,
        plugins: driver.settled_names().to_vec(),
    };

    let ctx: CompileContext<'_> = CompileContext::new(
        resolved.cwd.as_deref().unwrap_or("/"),
        &resolved.file,
        &resolved.code,
    );

    let module: telarel_plugin::ModuleInfo<'_> = telarel_plugin::ModuleInfo {
        file: &resolved.file,
        code: &resolved.code,
        language,
        source_type,
    };

    let plugin_ctx: PluginContext<'_> =
        PluginContext::new(&common.state, ctx.cwd, module);

    // R5 steps 4-10 with the error path: `compile_end` runs on EVERY path
    // after the settle (`err` set, last-good `code`/`map` on error); the
    // ORIGINAL error still aborts with its existing stage-wrapped shape.
    let (code, map): (String, SourceMap) = match run_stages(
        &driver,
        &plugin_ctx,
        &resolved_options,
        &ctx,
        &resolved,
    )
    .await
    {
        | Ok(tail) => (tail.code, tail.map),
        | Err(stage) => {
            let args: CompileEndArgs = CompileEndArgs {
                code: stage.last_good.code,
                map: stage.last_good.map,
                err: Some(stage.error.to_string()),
            };

            // On an error path, the original error wins;
            // a compile_end failure is appended to its message, never
            // swallowed silently. The driver's error is context-wrapped
            // with the failing plugin name, so it is carried through here.
            if let Err(end_error) = driver.compile_end(&plugin_ctx, &args).await
            {
                return Err(CompileError::from_message(&format!(
                    "{} (compile_end hook failed: {end_error:#})",
                    stage.error
                )));
            }

            return Err(stage.error);
        },
    };

    // R5 step 10 on the success path: `compile_end` with `err` unset; if the
    // hook itself errors, the compile aborts with the stage-wrapped shape.
    driver
        .compile_end(
            &plugin_ctx,
            &CompileEndArgs {
                code: code.clone(),
                map: Some(map.clone()),
                err: None,
            },
        )
        .await
        .map_err(|error| {
            CompileError::from_message(&format!("compile_end hook: {error:#}"))
        })?;

    Ok(CompileOutput { code, map })
}

// The house style for the test hook impls is the explicit RPITIT form
// (never `async fn`), so the `manual_async_fn` lint is suppressed here.
#[cfg(test)]
#[allow(clippy::manual_async_fn)]
mod tests {
    use std::borrow::Cow;

    use telarel_common::{CompileOptions, ParseResult};
    use telarel_plugin::{
        Plugin, SharedPluginable, TransformArgs, TransformReturn,
    };

    use oxc::allocator::CloneIn;
    use oxc::ast_visit::VisitMut;
    use oxc::ast_visit::walk_mut;

    use oxc::ast::ast::{Directive, IdentifierReference, StringLiteral};
    use oxc::ast::builder::AstBuilder;
    use oxc::span::SPAN;
    use oxc::str::Ident;
    use oxc_sourcemap::SourceMapBuilder;

    use super::*;

    /// Build a single-source owned map with `(dst_line, dst_col,
    /// src_line, src_col)` tokens; the builder used for hook-returned
    /// incremental maps in the composition tests.
    fn hook_map(tokens: &[(u32, u32, u32, u32)]) -> SourceMap {
        let mut builder: SourceMapBuilder<'_> = SourceMapBuilder::default();

        builder.set_file("index.ts");

        let source_id: u32 = builder.add_source_and_content("index.ts", "hook");

        for &(dst_line, dst_col, src_line, src_col) in tokens {
            builder.add_token(
                dst_line,
                dst_col,
                src_line,
                src_col,
                Some(source_id),
                None,
            );
        }

        builder.into_owned_sourcemap().into_inner()
    }

    #[derive(Debug)]
    struct NoopPlugin;

    impl Plugin for NoopPlugin {
        fn name(&self) -> Cow<'static, str> {
            "noop".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Prepare
        }
    }

    const TARGET: &str = "console";

    const REPLACEMENT: &str = "consolex";

    const DIRECTIVE: &str = "x-telarel-chain";

    const REPLACED_CODE: &str = "console.log(2);";

    #[derive(Debug)]
    struct ReplaceRootWithConsolePlugin;

    impl Plugin for ReplaceRootWithConsolePlugin {
        fn name(&self) -> Cow<'static, str> {
            "replace-root-with-console".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Transform
        }

        // Replace the whole root with a freshly parsed program and return
        // `Some`: the replaced program becomes the carried one.
        fn transform<'a, 'ast: 'a>(
            &'a self,
            _ctx: &'a PluginContext<'a>,
            args: TransformArgs<'ast>,
        ) -> impl Future<Output = TransformReturn<'ast>> + 'a {
            async move {
                let code: &'ast str = args.allocator.alloc_str(REPLACED_CODE);

                let file: &'ast str =
                    args.allocator.alloc_str(_ctx.module.file);

                let ctx: CompileContext<'_> =
                    CompileContext::new("/repo", file, code);

                let parsed: ParseResult<'_> = parse(ParseOptions {
                    context: &ctx,
                    allocator: args.allocator,
                    file,
                    code,
                    language: None,
                    source_type: None,
                })?;

                let fresh: &'ast Program<'ast> =
                    args.allocator.alloc(parsed.program);

                Ok(Some(telarel_plugin::TransformOutput { ast: fresh }))
            }
        }
    }

    struct Renamer<'x> {
        allocator: &'x Allocator,
    }

    impl<'x> VisitMut<'x> for Renamer<'x> {
        fn visit_identifier_reference(
            &mut self,
            ident: &mut IdentifierReference<'x>,
        ) {
            if ident.name.as_str() == TARGET {
                ident.name = Ident::from_str_in(REPLACEMENT, &self.allocator);
            }
        }
    }

    #[derive(Debug)]
    struct RenameCalleePlugin;

    impl Plugin for RenameCalleePlugin {
        fn name(&self) -> Cow<'static, str> {
            "rename-callee".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Transform
        }

        // Clone the read-only program into the compile allocator, rename
        // every `console` IdentifierReference to `consolex` on the clone,
        // and return it as the replacement.
        fn transform<'a, 'ast: 'a>(
            &'a self,
            _ctx: &'a PluginContext<'a>,
            args: TransformArgs<'ast>,
        ) -> impl Future<Output = TransformReturn<'ast>> + 'a {
            async move {
                let mut working: Program<'ast> =
                    (*args.ast).clone_in(args.allocator);

                let mut renamer: Renamer<'ast> =
                    Renamer { allocator: args.allocator };

                walk_mut::walk_program(&mut renamer, &mut working);

                let rooted: &'ast Program<'ast> = args.allocator.alloc(working);

                Ok(Some(telarel_plugin::TransformOutput { ast: rooted }))
            }
        }
    }

    #[derive(Debug)]
    struct AppendDirectivePlugin;

    impl Plugin for AppendDirectivePlugin {
        fn name(&self) -> Cow<'static, str> {
            "append-directive".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Transform
        }

        // Clone the read-only program into the compile allocator and append
        // one extra directive to the clone, then return it.
        fn transform<'a, 'ast: 'a>(
            &'a self,
            _ctx: &'a PluginContext<'a>,
            args: TransformArgs<'ast>,
        ) -> impl Future<Output = TransformReturn<'ast>> + 'a {
            async move {
                let mut working: Program<'ast> =
                    (*args.ast).clone_in(args.allocator);

                let builder: AstBuilder<'ast> = AstBuilder::new(args.allocator);

                let string_literal: StringLiteral<'ast> =
                    StringLiteral::new(SPAN, DIRECTIVE, None, &builder);

                let directive: Directive<'ast> =
                    Directive::new(SPAN, string_literal, DIRECTIVE, &builder);

                working.directives.push(directive);

                let rooted: &'ast Program<'ast> = args.allocator.alloc(working);

                Ok(Some(telarel_plugin::TransformOutput { ast: rooted }))
            }
        }
    }

    #[derive(Debug)]
    struct NoopTransformPlugin;

    impl Plugin for NoopTransformPlugin {
        fn name(&self) -> Cow<'static, str> {
            "noop-transform".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Transform
        }

        // A transform hook that changes nothing: returns `None`, so the
        // parsed original is carried unchanged to the rest of the chain.
        fn transform<'a, 'ast: 'a>(
            &'a self,
            _ctx: &'a PluginContext<'a>,
            _args: TransformArgs<'ast>,
        ) -> impl Future<Output = TransformReturn<'ast>> + 'a {
            async move { Ok(None) }
        }
    }

    #[derive(Debug)]
    struct RewriteCodeOptionsPlugin;

    impl Plugin for RewriteCodeOptionsPlugin {
        fn name(&self) -> Cow<'static, str> {
            "rewrite-code-options".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Options
        }

        fn options<'a>(
            &'a self,
            _ctx: &'a telarel_plugin::CommonPluginContext,
            args: &'a telarel_plugin::OptionsArgs,
        ) -> impl Future<Output = telarel_plugin::OptionsReturn> + Send
        {
            async move {
                let mut next: telarel_plugin::OptionsArgs = args.clone();

                next.options.code = "const rewritten = 7;".to_string();

                Ok(Some(next))
            }
        }
    }

    #[derive(Debug)]
    struct ObserveCwdOptionsPlugin {
        observed: std::sync::Arc<std::sync::Mutex<Vec<Option<String>>>>,
    }

    impl Plugin for ObserveCwdOptionsPlugin {
        fn name(&self) -> Cow<'static, str> {
            "observe-cwd-options".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Options
        }

        fn options<'a>(
            &'a self,
            _ctx: &'a telarel_plugin::CommonPluginContext,
            args: &'a telarel_plugin::OptionsArgs,
        ) -> impl Future<Output = telarel_plugin::OptionsReturn> + Send
        {
            async move {
                self.observed.lock().unwrap().push(args.options.cwd.clone());

                Ok(None)
            }
        }
    }

    #[derive(Debug)]
    struct DropCwdOptionsPlugin;

    impl Plugin for DropCwdOptionsPlugin {
        fn name(&self) -> Cow<'static, str> {
            "drop-cwd-options".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Options
        }

        fn options<'a>(
            &'a self,
            _ctx: &'a telarel_plugin::CommonPluginContext,
            args: &'a telarel_plugin::OptionsArgs,
        ) -> impl Future<Output = telarel_plugin::OptionsReturn> + Send
        {
            async move {
                let mut next: telarel_plugin::OptionsArgs = args.clone();

                next.options.cwd = None;

                Ok(Some(next))
            }
        }
    }

    #[derive(Debug)]
    struct ObserveCwdCompileStartPlugin {
        observed: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    }

    impl Plugin for ObserveCwdCompileStartPlugin {
        fn name(&self) -> Cow<'static, str> {
            "observe-cwd-compile-start".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::CompileStart
        }

        fn compile_start<'a>(
            &'a self,
            _ctx: &'a telarel_plugin::PluginContext<'_>,
            args: &'a telarel_plugin::CompileStartArgs,
        ) -> impl Future<Output = telarel_plugin::NotifyReturn> + Send {
            let cwd: String = args.options.cwd.clone();

            async move {
                self.observed.lock().unwrap().push(cwd);

                Ok(())
            }
        }
    }

    #[derive(Debug)]
    struct FailingPreparePlugin;

    impl Plugin for FailingPreparePlugin {
        fn name(&self) -> Cow<'static, str> {
            "failing-prepare".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Prepare
        }

        fn prepare<'a>(
            &'a self,
            _ctx: &'a telarel_plugin::PluginContext<'_>,
            _args: &'a telarel_plugin::PrepareArgs<'_>,
        ) -> impl Future<Output = telarel_plugin::PrepareReturn> + Send
        {
            async { Err(anyhow::anyhow!("stage boom")) }
        }
    }

    #[derive(Debug)]
    struct FailingFinalizePlugin;

    impl Plugin for FailingFinalizePlugin {
        fn name(&self) -> Cow<'static, str> {
            "failing-finalize".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Finalize
        }

        fn finalize<'a>(
            &'a self,
            _ctx: &'a telarel_plugin::PluginContext<'_>,
            _args: &'a telarel_plugin::FinalizeArgs<'_>,
        ) -> impl Future<Output = telarel_plugin::FinalizeReturn> + Send
        {
            async { Err(anyhow::anyhow!("stage boom")) }
        }
    }

    #[derive(Debug)]
    struct FailingOptionsPlugin;

    impl Plugin for FailingOptionsPlugin {
        fn name(&self) -> Cow<'static, str> {
            "failing-options".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Options
        }

        fn options<'a>(
            &'a self,
            _ctx: &'a telarel_plugin::CommonPluginContext,
            _args: &'a telarel_plugin::OptionsArgs,
        ) -> impl Future<Output = telarel_plugin::OptionsReturn> + Send
        {
            async { Err(anyhow::anyhow!("stage boom")) }
        }
    }

    // Injects a plugin into the args' plugin list ONCE (first invocation):
    // the fixpoint must run the injected plugin's `options` hook in the
    // next pass and the settled list (hence the parse-skip gate) must
    // include it. Injecting on every invocation would be a plugin cycle,
    // correctly aborted by the depth guard.
    #[derive(Debug)]
    struct InjectOptionsPlugin {
        injected: SharedPluginable,
        remaining: std::sync::Mutex<usize>,
    }

    impl Plugin for InjectOptionsPlugin {
        fn name(&self) -> Cow<'static, str> {
            "inject-options".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Options
        }

        fn options<'a>(
            &'a self,
            _ctx: &'a telarel_plugin::CommonPluginContext,
            args: &'a telarel_plugin::OptionsArgs,
        ) -> impl Future<Output = telarel_plugin::OptionsReturn> + Send
        {
            async move {
                {
                    let mut remaining: std::sync::MutexGuard<'_, usize> =
                        self.remaining.lock().unwrap();

                    if *remaining == 0 {
                        return Ok(None);
                    }

                    *remaining -= 1;
                }

                let mut next: telarel_plugin::OptionsArgs = args.clone();

                next.plugins.push(self.injected.clone());

                Ok(Some(next))
            }
        }
    }

    fn options() -> CompileOptions {
        CompileOptions {
            file: "index.ts".to_string(),
            code: "const a = 1;".to_string(),
            ..Default::default()
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_no_plugins_codegens_original() {
        let out: crate::CompileOutput =
            compile(options(), vec![]).await.unwrap();

        assert_eq!(out.code, "const a = 1;");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_noop_plugin_leaves_output_unchanged() {
        let opts: CompileOptions = CompileOptions {
            file: "index.ts".to_string(),
            code: "console.log(1);".to_string(),
            ..Default::default()
        };

        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(NoopPlugin)];

        let out = compile(opts, plugins).await.unwrap();

        assert_eq!(out.code, "console.log(1);");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_strips_trailing_newline() {
        let opts: CompileOptions = CompileOptions {
            file: "index.ts".to_string(),
            code: "const a = 1;\n".to_string(),
            ..Default::default()
        };

        let out: crate::CompileOutput = compile(opts, vec![]).await.unwrap();

        assert_eq!(out.code, "const a = 1;");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_strips_crlf_trailing_newline() {
        // CRLF sources must not leave a stray `\r` after the `\n` strip.
        let opts: CompileOptions = CompileOptions {
            file: "index.ts".to_string(),
            code: "const a = 1;\r\n".to_string(),
            ..Default::default()
        };

        let out: crate::CompileOutput = compile(opts, vec![]).await.unwrap();

        assert_eq!(out.code, "const a = 1;");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_preserves_interior_crlf() {
        // Only the trailing terminator is stripped; interior CRLF is intact.
        let opts: CompileOptions = CompileOptions {
            file: "index.ts".to_string(),
            code: "const a = 1;\r\nconst b = 2;".to_string(),
            ..Default::default()
        };

        let out: crate::CompileOutput = compile(opts, vec![]).await.unwrap();

        assert_eq!(out.code, "const a = 1;\r\nconst b = 2;");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_strips_lone_cr_trailing_newline() {
        // A lone `\r` terminator is also stripped, matching the documented
        // behavior of the trim (`\n`, `\r\n`, and lone `\r`).
        let opts: CompileOptions = CompileOptions {
            file: "index.ts".to_string(),
            code: "const a = 1;\r".to_string(),
            ..Default::default()
        };

        let out: crate::CompileOutput = compile(opts, vec![]).await.unwrap();

        assert_eq!(out.code, "const a = 1;");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_no_plugins_sourcemap_sources() {
        let out: crate::CompileOutput =
            compile(options(), vec![]).await.unwrap();

        assert_eq!(out.map.get_source(0), Some("index.ts"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_transform_plugin_replaces() {
        let opts: CompileOptions = CompileOptions {
            file: "index.ts".to_string(),
            code: "console.log(1);".to_string(),
            ..Default::default()
        };

        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(RenameCalleePlugin)];

        let out = compile(opts, plugins).await.unwrap();

        assert!(out.code.contains("consolex"), "{}", out.code);

        let map: SourceMap = out.map;

        assert!(
            map.get_tokens().next().is_some(),
            "mappings must be non-empty"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_transform_sourcemap_points_at_original() {
        let opts: CompileOptions = CompileOptions {
            file: "index.ts".to_string(),
            code: "console.log(1);".to_string(),
            ..Default::default()
        };

        let out = compile(opts, vec![Plugin::new_shared(RenameCalleePlugin)])
            .await
            .unwrap();

        let map: SourceMap = out.map;

        assert_eq!(map.get_source(0), Some("index.ts"));
        assert!(
            map.get_tokens().next().is_some(),
            "mappings must be non-empty"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_invalid_code_errors_with_transform_plugin() {
        let opts: CompileOptions = CompileOptions {
            file: "index.ts".to_string(),
            code: "const = ;".to_string(),
            ..Default::default()
        };

        let error: CompileError =
            compile(opts, vec![Plugin::new_shared(RenameCalleePlugin)])
                .await
                .unwrap_err();

        assert!(error.to_string().contains("index.ts"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_no_transform_usage_skips_parse() {
        // Invalid syntax passes through unparsed when no plugin uses
        // `transform` — the skip path never parses.
        let opts: CompileOptions = CompileOptions {
            file: "index.ts".to_string(),
            code: "const = ;".to_string(),
            ..Default::default()
        };

        let out: crate::CompileOutput = compile(opts, vec![]).await.unwrap();

        assert_eq!(out.code, "const = ;");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_no_transform_usage_identity_map() {
        let opts: CompileOptions = CompileOptions {
            file: "index.ts".to_string(),
            code: "const a = 1;\nconst b = 2;\n".to_string(),
            ..Default::default()
        };

        let out: crate::CompileOutput = compile(opts, vec![]).await.unwrap();

        assert_eq!(out.code, "const a = 1;\nconst b = 2;");
        assert_eq!(out.map.get_source(0), Some("index.ts"));

        let token: oxc_sourcemap::Token =
            out.map.get_token(1).expect("one token per line");

        assert_eq!(token.get_dst_line(), 1);
        assert_eq!(token.get_src_line(), 1);
        assert_eq!(token.get_dst_col(), 0);
        assert_eq!(token.get_src_col(), 0);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_noop_prepare_plugin_does_not_parse() {
        // A plugin declaring only PRE must not force the parse path; output
        // stays verbatim (proves NoopPlugin's PRE declaration doesn't leak
        // into the transform decision).
        let opts: CompileOptions = CompileOptions {
            file: "index.ts".to_string(),
            code: "console.log(1);\n".to_string(),
            ..Default::default()
        };

        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(NoopPlugin)];

        let out = compile(opts, plugins).await.unwrap();

        assert_eq!(out.code, "console.log(1);");
        assert_eq!(out.map.get_source(0), Some("index.ts"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_options_hook_rewrites_code() {
        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(RewriteCodeOptionsPlugin)];

        let out: crate::CompileOutput =
            compile(options(), plugins).await.unwrap();

        assert!(out.code.contains("rewritten"), "{}", out.code);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_prepare_hook_error_aborts() {
        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(FailingPreparePlugin)];

        let error: CompileError =
            compile(options(), plugins).await.unwrap_err();

        let message: String = error.to_string();

        assert!(message.contains("prepare hook"), "{}", message);
        assert!(message.contains("stage boom"), "{}", message);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_output_map_is_alias() {
        // `SourceMap` in `CompileOutput` must be the owned-map alias: assign it
        // to an explicitly-aliased variable to prove the types agree.
        let opts: CompileOptions = CompileOptions {
            file: "index.ts".to_string(),
            code: "const a = 1;".to_string(),
            ..Default::default()
        };

        let out: crate::CompileOutput = compile(opts, vec![]).await.unwrap();

        let map: SourceMap = out.map;

        assert_eq!(map.get_source(0), Some("index.ts"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_finalize_hook_error_aborts() {
        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(FailingFinalizePlugin)];

        let error: CompileError =
            compile(options(), plugins).await.unwrap_err();

        let message: String = error.to_string();

        assert!(message.contains("finalize hook"), "{}", message);
        assert!(message.contains("stage boom"), "{}", message);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_transform_chain_multiple_replacements() {
        // Two replacing plugins in sequence: the rename must survive into the
        // second plugin and both transformations must appear in the final
        // code (proves the chain carries each replacement by reference).
        let opts: CompileOptions = CompileOptions {
            file: "index.ts".to_string(),
            code: "console.log(1);".to_string(),
            ..Default::default()
        };

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(RenameCalleePlugin),
            Plugin::new_shared(AppendDirectivePlugin),
        ];

        let out = compile(opts, plugins).await.unwrap();

        assert!(out.code.contains("consolex"), "{}", out.code);
        assert!(out.code.contains("x-telarel-chain"), "{}", out.code);
        assert_eq!(out.map.get_source(0), Some("index.ts"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_transform_some_then_second_replacement_kept() {
        // Some(replace root) followed by a second replacing plugin: the
        // final codegen must contain BOTH replacements (the second plugin
        // receives the first's returned program).
        let opts: CompileOptions = CompileOptions {
            file: "index.ts".to_string(),
            code: "console.log(1);".to_string(),
            ..Default::default()
        };

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(ReplaceRootWithConsolePlugin),
            Plugin::new_shared(RenameCalleePlugin),
        ];

        let out: CompileOutput = compile(opts, plugins).await.unwrap();

        // The replaced root's `console` callee is renamed by the second
        // plugin: both edits must be visible as `consolex.log(2);`.
        assert!(out.code.contains("consolex.log(2)"), "{}", out.code);
        assert_eq!(out.map.get_source(0), Some("index.ts"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_transform_noop_plugin_keeps_original() {
        // A transform plugin returning `None` must leave the parsed original
        // to codegen: the code is unchanged verbatim.
        let opts: CompileOptions = CompileOptions {
            file: "index.ts".to_string(),
            code: "console.log(1);".to_string(),
            ..Default::default()
        };

        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(NoopTransformPlugin)];

        let out = compile(opts, plugins).await.unwrap();

        assert!(out.code.contains("console.log(1);"), "{}", out.code);
        assert_eq!(out.map.get_source(0), Some("index.ts"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_transform_mixed_chain_noop_then_replace() {
        // Noop, replace, noop: only the replacing plugin's mutation must
        // reach codegen, and the surrounding noop plugins must not reset
        // the tree to the original.
        let opts: CompileOptions = CompileOptions {
            file: "index.ts".to_string(),
            code: "console.log(1);".to_string(),
            ..Default::default()
        };

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(NoopTransformPlugin),
            Plugin::new_shared(RenameCalleePlugin),
            Plugin::new_shared(NoopTransformPlugin),
        ];

        let out = compile(opts, plugins).await.unwrap();

        assert!(out.code.contains("consolex"), "{}", out.code);
        assert_eq!(out.map.get_source(0), Some("index.ts"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_omitted_cwd_resolves_to_process_cwd() {
        let observed: std::sync::Arc<std::sync::Mutex<Vec<Option<String>>>> =
            std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(ObserveCwdOptionsPlugin {
                observed: std::sync::Arc::clone(&observed),
            })];

        let opts: CompileOptions = CompileOptions {
            file: "index.ts".to_string(),
            code: "const a = 1;".to_string(),
            ..Default::default()
        };

        compile(opts, plugins).await.unwrap();

        let expected: Option<String> = Some(
            std::env::current_dir()
                .map(|p: std::path::PathBuf| p.to_string_lossy().into_owned())
                .unwrap_or_else(|_| String::from("/")),
        );

        let recorded: Vec<Option<String>> = observed.lock().unwrap().clone();

        assert_eq!(recorded, vec![expected]);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_late_options_cwd_omission_resolves_to_process_cwd() {
        let observed: std::sync::Arc<std::sync::Mutex<Vec<String>>> =
            std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(DropCwdOptionsPlugin),
            Plugin::new_shared(ObserveCwdCompileStartPlugin {
                observed: std::sync::Arc::clone(&observed),
            }),
        ];

        let opts: CompileOptions = CompileOptions {
            file: "index.ts".to_string(),
            code: "const a = 1;".to_string(),
            ..Default::default()
        };

        compile(opts, plugins).await.unwrap();

        let recorded: Vec<String> = observed.lock().unwrap().clone();

        assert_eq!(recorded, vec![resolve_process_cwd()]);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_options_injected_transform_plugin_triggers_parse() {
        // A plugin injected via the `options` fixpoint declares Transform:
        // the parse-skip gate is computed from the SETTLED list, so the
        // gate must open (parse runs) even though the initial list was
        // transform-free. `RenameCalleePlugin` only runs on the parse path,
        // so seeing `consolex` proves parse+transform+codegen executed.
        let opts: CompileOptions = CompileOptions {
            file: "index.ts".to_string(),
            code: "console.log(1);".to_string(),
            ..Default::default()
        };

        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(InjectOptionsPlugin {
                injected: Plugin::new_shared(RenameCalleePlugin),
                remaining: std::sync::Mutex::new(1),
            })];

        let out: CompileOutput = compile(opts, plugins).await.unwrap();

        assert!(out.code.contains("consolex"), "{}", out.code);
        assert_eq!(out.map.get_source(0), Some("index.ts"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_options_fixpoint_skips_parse_without_transform() {
        // The skip path is retained: an options-only plugin that injects a
        // PRE-declaring plugin must NOT trigger parse — invalid syntax still
        // passes through unparsed, and the identity map is emitted.
        let opts: CompileOptions = CompileOptions {
            file: "index.ts".to_string(),
            code: "const = ;".to_string(),
            ..Default::default()
        };

        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(InjectOptionsPlugin {
                injected: Plugin::new_shared(NoopPlugin),
                remaining: std::sync::Mutex::new(1),
            })];

        let out: crate::CompileOutput = compile(opts, plugins).await.unwrap();

        assert_eq!(out.code, "const = ;");

        let token: oxc_sourcemap::Token =
            out.map.get_token(0).expect("identity map line token");

        assert_eq!(token.get_dst_line(), 0);
        assert_eq!(token.get_src_line(), 0);
        assert_eq!(token.get_dst_col(), 0);
        assert_eq!(token.get_src_col(), 0);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_options_hook_error_aborts_with_stage_prefix() {
        // A per-plugin options error composes to
        // `options hook: \`{name}\` options: {err}` — the stage prefix wraps
        // the runner's per-plugin context exactly once.
        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(FailingOptionsPlugin)];

        let error: CompileError =
            compile(options(), plugins).await.unwrap_err();

        let message: String = error.to_string();

        assert!(message.contains("options hook"), "{}", message);
        assert!(message.contains("`failing-options` options"), "{}", message);
        assert!(message.contains("stage boom"), "{}", message);
    }

    #[derive(Debug)]
    struct ObserveCompileStartPlugin {
        seen: std::sync::Arc<std::sync::Mutex<Vec<ResolvedOptions>>>,
    }

    impl Plugin for ObserveCompileStartPlugin {
        fn name(&self) -> Cow<'static, str> {
            "observe-compile-start".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::CompileStart
        }

        fn compile_start<'a>(
            &'a self,
            _ctx: &'a telarel_plugin::PluginContext<'_>,
            args: &'a telarel_plugin::CompileStartArgs,
        ) -> impl Future<Output = telarel_plugin::NotifyReturn> + Send {
            let options: ResolvedOptions = args.options.clone();

            async move {
                self.seen.lock().unwrap().push(options);

                Ok(())
            }
        }
    }

    /// A captured `compile_end` payload.
    #[derive(Debug, Clone)]
    struct CompileEndShape {
        code: String,
        err: Option<String>,
        has_map: bool,
    }

    #[derive(Debug)]
    struct ObserveCompileEndPlugin {
        seen: std::sync::Arc<std::sync::Mutex<Vec<CompileEndShape>>>,
    }

    impl Plugin for ObserveCompileEndPlugin {
        fn name(&self) -> Cow<'static, str> {
            "observe-compile-end".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::CompileEnd
        }

        fn compile_end<'a>(
            &'a self,
            _ctx: &'a telarel_plugin::PluginContext<'_>,
            args: &'a telarel_plugin::CompileEndArgs,
        ) -> impl Future<Output = telarel_plugin::NotifyReturn> + Send {
            let shape: CompileEndShape = CompileEndShape {
                code: args.code.clone(),
                err: args.err.clone(),
                has_map: args.map.is_some(),
            };

            async move {
                self.seen.lock().unwrap().push(shape);

                Ok(())
            }
        }
    }

    // A `prepare` hook that errors; used to verify the error-path `compile_end`.
    #[derive(Debug)]
    struct CompileEndOnPrepareErrorPlugin {
        seen: std::sync::Arc<std::sync::Mutex<Vec<CompileEndShape>>>,
    }

    impl Plugin for CompileEndOnPrepareErrorPlugin {
        fn name(&self) -> Cow<'static, str> {
            "compile-end-on-prepare-error".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Prepare | HookUsage::CompileEnd
        }

        fn prepare<'a>(
            &'a self,
            _ctx: &'a telarel_plugin::PluginContext<'_>,
            _args: &'a telarel_plugin::PrepareArgs<'_>,
        ) -> impl Future<Output = telarel_plugin::PrepareReturn> + Send
        {
            async { Err(anyhow::anyhow!("stage boom")) }
        }

        fn compile_end<'a>(
            &'a self,
            _ctx: &'a telarel_plugin::PluginContext<'_>,
            args: &'a telarel_plugin::CompileEndArgs,
        ) -> impl Future<Output = telarel_plugin::NotifyReturn> + Send {
            let shape: CompileEndShape = CompileEndShape {
                code: args.code.clone(),
                err: args.err.clone(),
                has_map: args.map.is_some(),
            };

            async move {
                self.seen.lock().unwrap().push(shape);

                Ok(())
            }
        }
    }

    // An `options` hook that errors; the pipeline aborts BEFORE the driver
    // exists, so `compile_end` cannot run for this path (documented
    // boundary: the settled list does not exist yet).
    #[derive(Debug)]
    struct CompileEndNeverOptionsErrorPlugin {
        end_invoked: std::sync::Arc<std::sync::Mutex<bool>>,
    }

    impl Plugin for CompileEndNeverOptionsErrorPlugin {
        fn name(&self) -> Cow<'static, str> {
            "end-never-options-error".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Options | HookUsage::CompileEnd
        }

        fn options<'a>(
            &'a self,
            _ctx: &'a telarel_plugin::CommonPluginContext,
            _args: &'a telarel_plugin::OptionsArgs,
        ) -> impl Future<Output = telarel_plugin::OptionsReturn> + Send
        {
            async { Err(anyhow::anyhow!("stage boom")) }
        }

        fn compile_end<'a>(
            &'a self,
            _ctx: &'a telarel_plugin::PluginContext<'_>,
            _args: &'a telarel_plugin::CompileEndArgs,
        ) -> impl Future<Output = telarel_plugin::NotifyReturn> + Send {
            let flag: std::sync::Arc<std::sync::Mutex<bool>> =
                std::sync::Arc::clone(&self.end_invoked);

            async move {
                *flag.lock().unwrap() = true;

                Ok(())
            }
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_start_sees_fully_resolved_options() {
        let seen: std::sync::Arc<std::sync::Mutex<Vec<ResolvedOptions>>> =
            std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(NoopPlugin),
            Plugin::new_shared(ObserveCompileStartPlugin {
                seen: std::sync::Arc::clone(&seen),
            }),
        ];

        let opts: CompileOptions = CompileOptions {
            file: "index.ts".to_string(),
            code: "const a = 1;".to_string(),
            ..Default::default()
        };

        compile(opts, plugins).await.unwrap();

        let recorded: Vec<ResolvedOptions> = seen.lock().unwrap().clone();

        assert_eq!(recorded.len(), 1, "compile_start ran once");

        let options: &ResolvedOptions = &recorded[0];

        assert_eq!(
            options.cwd,
            std::env::current_dir()
                .map(|p: std::path::PathBuf| p.to_string_lossy().into_owned())
                .unwrap_or_else(|_| String::from("/"))
                .as_str(),
        );

        assert_eq!(options.file, "index.ts");
        assert_eq!(options.code, "const a = 1;");

        assert_eq!(options.language, Language::TS);

        assert_eq!(
            options.plugins,
            vec![String::from("noop"), String::from("observe-compile-start"),],
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_end_runs_on_success_and_error_path() {
        // Success: compile_end runs with err unset and the final code.
        let seen: std::sync::Arc<std::sync::Mutex<Vec<CompileEndShape>>> =
            std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));

        let observer: SharedPluginable =
            Plugin::new_shared(ObserveCompileEndPlugin {
                seen: std::sync::Arc::clone(&seen),
            });

        let opts: CompileOptions = CompileOptions {
            file: "index.ts".to_string(),
            code: "console.log(1);".to_string(),
            ..Default::default()
        };

        let out: CompileOutput = compile(
            opts.clone(),
            vec![observer, Plugin::new_shared(RenameCalleePlugin)],
        )
        .await
        .unwrap();

        let recorded: Vec<CompileEndShape> = seen.lock().unwrap().clone();

        assert_eq!(recorded.len(), 1, "compile_end ran once on success");

        assert!(recorded[0].err.is_none(), "{}", recorded[0].code);
        assert!(recorded[0].has_map);
        assert_eq!(recorded[0].code, out.code);

        // Error: a failing `prepare` hook must still run `compile_end` with
        // `err` set and the last-good code/map (the untouched source, no
        // map yet).
        let error_seen: std::sync::Arc<std::sync::Mutex<Vec<CompileEndShape>>> =
            std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));

        let opts: CompileOptions = CompileOptions {
            file: "index.ts".to_string(),
            code: "const a = 1;".to_string(),
            ..Default::default()
        };

        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(CompileEndOnPrepareErrorPlugin {
                seen: std::sync::Arc::clone(&error_seen),
            })];

        let error: CompileError = compile(opts, plugins).await.unwrap_err();

        let message: String = error.to_string();

        assert!(message.contains("prepare hook"), "{}", message);
        assert!(message.contains("stage boom"), "{}", message);

        let recorded: Vec<CompileEndShape> = error_seen.lock().unwrap().clone();

        assert_eq!(recorded.len(), 1, "compile_end ran on the error path");

        assert_eq!(
            recorded[0].err.as_deref(),
            Some(
                "prepare hook: `compile-end-on-prepare-error` prepare: stage boom"
            ),
        );
        assert_eq!(recorded[0].code, "const a = 1;");
        assert!(!recorded[0].has_map);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_start_sees_resolved_source_type_cjs() {
        // The sourceType is resolved from the resolved grammar's module
        // kind.
        let seen: std::sync::Arc<std::sync::Mutex<Vec<ResolvedOptions>>> =
            std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(ObserveCompileStartPlugin {
                seen: std::sync::Arc::clone(&seen),
            })];

        let opts: CompileOptions = CompileOptions {
            file: "index.cjs".to_string(),
            code: "module.exports = 1;".to_string(),
            ..Default::default()
        };

        compile(opts, plugins).await.unwrap();

        let recorded: Vec<ResolvedOptions> = seen.lock().unwrap().clone();

        assert_eq!(recorded[0].language, Language::JS);
        assert_eq!(recorded[0].source_type, SourceType::CommonJS);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_options_error_path_cannot_run_compile_end() {
        // The options fixpoint aborts BEFORE the driver exists — the
        // settled list is unknowable, so `compile_end` cannot run for this
        // path (documented R5 boundary; the whole-body wrap covers the
        // settled pipeline only).
        let invoked: std::sync::Arc<std::sync::Mutex<bool>> =
            std::sync::Arc::new(std::sync::Mutex::new(false));

        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(CompileEndNeverOptionsErrorPlugin {
                end_invoked: std::sync::Arc::clone(&invoked),
            })];

        let error: CompileError =
            compile(options(), plugins).await.unwrap_err();

        let message: String = error.to_string();

        assert!(message.contains("options hook"), "{}", message);

        assert!(!*invoked.lock().unwrap(), "compile_end must not run");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_end_runs_on_parse_error_path() {
        // A failing parse stage still runs `compile_end` with `err` set.
        let seen: std::sync::Arc<std::sync::Mutex<Vec<CompileEndShape>>> =
            std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));

        // RenameCalleePlugin will fail in `transform` (`const = ;`
        // cannot parse); ObserveCompileEndPlugin marks that compile_end
        // ran on the error path.
        let opts: CompileOptions = CompileOptions {
            file: "index.ts".to_string(),
            code: "const = ;".to_string(),
            ..Default::default()
        };

        let observer: SharedPluginable =
            Plugin::new_shared(ObserveCompileEndPlugin {
                seen: std::sync::Arc::clone(&seen),
            });

        let error: CompileError = compile(
            opts.clone(),
            vec![observer, Plugin::new_shared(RenameCalleePlugin)],
        )
        .await
        .unwrap_err();

        let message: String = error.to_string();

        assert!(message.contains("index.ts"), "{}", message);

        let recorded: Vec<CompileEndShape> = seen.lock().unwrap().clone();

        assert_eq!(
            recorded.len(),
            1,
            "compile_end ran on the parse-error path"
        );
        assert!(recorded[0].err.is_some());
    }

    // Fixture returning a bare error in the RPITIT shape used by the
    // frozen plugin-crate fixtures (same `allow` pattern as there).
    #[derive(Debug)]
    struct FailingStartPlugin;

    impl Plugin for FailingStartPlugin {
        fn name(&self) -> Cow<'static, str> {
            "failing-start".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::CompileStart
        }

        fn compile_start<'a>(
            &'a self,
            _ctx: &'a telarel_plugin::PluginContext<'_>,
            _args: &'a telarel_plugin::CompileStartArgs,
        ) -> impl Future<Output = telarel_plugin::NotifyReturn> + Send {
            async { Err(anyhow::anyhow!("start boom")) }
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_start_error_runs_compile_end() {
        // An error in `compile_start` itself is a settled-pipeline error:
        // `compile_end` runs with `err` set before the original aborts.
        let seen: std::sync::Arc<std::sync::Mutex<Vec<CompileEndShape>>> =
            std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));

        let opts: CompileOptions = CompileOptions {
            file: "index.ts".to_string(),
            code: "const a = 1;".to_string(),
            ..Default::default()
        };

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(ObserveCompileEndPlugin {
                seen: std::sync::Arc::clone(&seen),
            }),
            Plugin::new_shared(FailingStartPlugin),
        ];

        let error: CompileError = compile(opts, plugins).await.unwrap_err();

        let message: String = error.to_string();

        assert!(message.contains("compile_start hook"), "{}", message);
        assert!(message.contains("start boom"), "{}", message);

        let recorded: Vec<CompileEndShape> = seen.lock().unwrap().clone();

        assert_eq!(
            recorded.len(),
            1,
            "compile_end ran on the start-error path"
        );
        assert!(
            recorded[0]
                .err
                .as_deref()
                .is_some_and(|err| err.contains("start boom")),
        );
    }

    #[derive(Debug)]
    struct FailingEndPlugin;

    impl Plugin for FailingEndPlugin {
        fn name(&self) -> Cow<'static, str> {
            "failing-end".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::CompileEnd
        }

        fn compile_end<'a>(
            &'a self,
            _ctx: &'a telarel_plugin::PluginContext<'_>,
            _args: &'a telarel_plugin::CompileEndArgs,
        ) -> impl Future<Output = telarel_plugin::NotifyReturn> + Send {
            async { Err(anyhow::anyhow!("end boom")) }
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_end_error_on_success_path_aborts() {
        // compile_end failing on the SUCCESS path aborts the compile with
        // the stage-wrapped shape.
        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(FailingEndPlugin)];

        let error: CompileError =
            compile(options(), plugins).await.unwrap_err();

        let message: String = error.to_string();

        assert!(message.contains("compile_end hook"), "{}", message);
        assert!(message.contains("`failing-end` compile_end"), "{}", message);
        assert!(message.contains("end boom"), "{}", message);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_end_error_on_error_path_original_wins() {
        // A failing stage AND a failing compile_end: the original error
        // wins, with the compile_end failure appended (nothing swallowed).
        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(FailingPreparePlugin),
            Plugin::new_shared(FailingEndPlugin),
        ];

        let error: CompileError =
            compile(options(), plugins).await.unwrap_err();

        let message: String = error.to_string();

        // The ORIGINAL error shape comes first, unchanged.
        assert!(
            message.starts_with(
                "prepare hook: `failing-prepare` prepare: stage boom"
            ),
            "{}",
            message
        );

        assert!(message.contains("compile_end hook failed"), "{}", message);

        // The appended driver error is context-wrapped with the failing
        // plugin's name, so the message names it (R3.2).
        assert!(message.contains("`failing-end` compile_end"), "{}", message);
    }

    // A prepare hook that replaces the source and returns an incremental map
    // over its own edit.
    #[derive(Debug)]
    struct PrepareWithMapPlugin {
        code: String,
        map: SourceMap,
    }

    impl Plugin for PrepareWithMapPlugin {
        fn name(&self) -> Cow<'static, str> {
            "prepare-with-map".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Prepare
        }

        fn prepare<'a>(
            &'a self,
            _ctx: &'a telarel_plugin::PluginContext<'_>,
            _args: &'a telarel_plugin::PrepareArgs<'_>,
        ) -> impl Future<Output = telarel_plugin::PrepareReturn> + Send
        {
            async move {
                Ok(Some(telarel_plugin::PrepareOutput {
                    code: self.code.clone(),
                    map: Some(self.map.clone()),
                }))
            }
        }
    }

    // A prepare hook that replaces the source and omits the map.
    #[derive(Debug)]
    struct PrepareNoMapPlugin {
        code: String,
    }

    impl Plugin for PrepareNoMapPlugin {
        fn name(&self) -> Cow<'static, str> {
            "prepare-no-map".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Prepare
        }

        fn prepare<'a>(
            &'a self,
            _ctx: &'a telarel_plugin::PluginContext<'_>,
            _args: &'a telarel_plugin::PrepareArgs<'_>,
        ) -> impl Future<Output = telarel_plugin::PrepareReturn> + Send
        {
            async move {
                Ok(Some(telarel_plugin::PrepareOutput {
                    code: self.code.clone(),
                    map: None,
                }))
            }
        }
    }

    // A finalize hook that replaces the generated code and returns an
    // incremental map over its own edit.
    #[derive(Debug)]
    struct FinalizeWithMapPlugin {
        code: String,
        map: SourceMap,
    }

    impl Plugin for FinalizeWithMapPlugin {
        fn name(&self) -> Cow<'static, str> {
            "finalize-with-map".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Finalize
        }

        fn finalize<'a>(
            &'a self,
            _ctx: &'a telarel_plugin::PluginContext<'_>,
            _args: &'a telarel_plugin::FinalizeArgs<'_>,
        ) -> impl Future<Output = telarel_plugin::FinalizeReturn> + Send
        {
            async move {
                Ok(Some(telarel_plugin::FinalizeOutput {
                    code: self.code.clone(),
                    map: Some(self.map.clone()),
                }))
            }
        }
    }

    // A finalize hook that replaces the generated code and omits the map.
    #[derive(Debug)]
    struct FinalizeNoMapPlugin {
        code: String,
    }

    impl Plugin for FinalizeNoMapPlugin {
        fn name(&self) -> Cow<'static, str> {
            "finalize-no-map".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Finalize
        }

        fn finalize<'a>(
            &'a self,
            _ctx: &'a telarel_plugin::PluginContext<'_>,
            _args: &'a telarel_plugin::FinalizeArgs<'_>,
        ) -> impl Future<Output = telarel_plugin::FinalizeReturn> + Send
        {
            async move {
                Ok(Some(telarel_plugin::FinalizeOutput {
                    code: self.code.clone(),
                    map: None,
                }))
            }
        }
    }

    // A prepare hook that ERRORS unless it receives exactly `expected_input`,
    // proving the fold carried the previous hook's rewrite; on match it
    // returns the configured code and optional incremental map. Built
    // relative to the received code, so a fixed-input fold cannot satisfy it.
    #[derive(Debug)]
    struct CarryingPreparePlugin {
        expected_input: String,
        output: String,
        map: Option<SourceMap>,
    }

    impl Plugin for CarryingPreparePlugin {
        fn name(&self) -> Cow<'static, str> {
            "carrying-prepare".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Prepare
        }

        fn prepare<'a>(
            &'a self,
            _ctx: &'a telarel_plugin::PluginContext<'_>,
            args: &'a telarel_plugin::PrepareArgs<'_>,
        ) -> impl Future<Output = telarel_plugin::PrepareReturn> + Send
        {
            async move {
                if args.code != self.expected_input {
                    anyhow::bail!(
                        "prepare hook received {:?}, expected the carried {:?}",
                        args.code,
                        self.expected_input,
                    );
                }

                Ok(Some(telarel_plugin::PrepareOutput {
                    code: self.output.clone(),
                    map: self.map.clone(),
                }))
            }
        }
    }

    // A prepare hook that records the code it received and returns `None`, so
    // the caller can assert the fold handed it the previous hook's output.
    #[derive(Debug)]
    struct PrepareProbePlugin {
        seen: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    }

    impl Plugin for PrepareProbePlugin {
        fn name(&self) -> Cow<'static, str> {
            "prepare-probe".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Prepare
        }

        fn prepare<'a>(
            &'a self,
            _ctx: &'a telarel_plugin::PluginContext<'_>,
            args: &'a telarel_plugin::PrepareArgs<'_>,
        ) -> impl Future<Output = telarel_plugin::PrepareReturn> + Send
        {
            async move {
                self.seen.lock().unwrap().push(args.code.to_string());

                Ok(None)
            }
        }
    }

    // A finalize hook that ERRORS unless it receives exactly `expected_input`,
    // proving the fold carried the previous hook's rewrite; on match it
    // returns the configured code and optional incremental map.
    #[derive(Debug)]
    struct CarryingFinalizePlugin {
        expected_input: String,
        output: String,
        map: Option<SourceMap>,
    }

    impl Plugin for CarryingFinalizePlugin {
        fn name(&self) -> Cow<'static, str> {
            "carrying-finalize".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Finalize
        }

        fn finalize<'a>(
            &'a self,
            _ctx: &'a telarel_plugin::PluginContext<'_>,
            args: &'a telarel_plugin::FinalizeArgs<'_>,
        ) -> impl Future<Output = telarel_plugin::FinalizeReturn> + Send
        {
            async move {
                if args.code != self.expected_input {
                    anyhow::bail!(
                        "finalize hook received {:?}, expected the carried {:?}",
                        args.code,
                        self.expected_input,
                    );
                }

                Ok(Some(telarel_plugin::FinalizeOutput {
                    code: self.output.clone(),
                    map: self.map.clone(),
                }))
            }
        }
    }

    // A finalize hook that records the code it received and returns `None`, so
    // the caller can assert the fold handed it the previous hook's output.
    #[derive(Debug)]
    struct FinalizeProbePlugin {
        seen: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    }

    impl Plugin for FinalizeProbePlugin {
        fn name(&self) -> Cow<'static, str> {
            "finalize-probe".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Finalize
        }

        fn finalize<'a>(
            &'a self,
            _ctx: &'a telarel_plugin::PluginContext<'_>,
            args: &'a telarel_plugin::FinalizeArgs<'_>,
        ) -> impl Future<Output = telarel_plugin::FinalizeReturn> + Send
        {
            async move {
                self.seen.lock().unwrap().push(args.code.to_string());

                Ok(None)
            }
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_prepare_none_is_noop() {
        // A prepare hook returning `None` never touches the source: the code
        // and map match the plugin-free compile.
        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(NoopPlugin)];

        let out: CompileOutput = compile(options(), plugins).await.unwrap();

        assert_eq!(out.code, "const a = 1;");
        assert_eq!(out.map.get_source(0), Some("index.ts"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_finalize_none_is_noop() {
        // A finalize hook returning `None` keeps the codegen code and map.
        let opts: CompileOptions = CompileOptions {
            file: "index.ts".to_string(),
            code: "console.log(1);".to_string(),
            ..Default::default()
        };

        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(NoopTransformPlugin)];

        let out: CompileOutput = compile(opts, plugins).await.unwrap();

        assert!(out.code.contains("console.log(1);"), "{}", out.code);
        assert_eq!(out.map.get_source(0), Some("index.ts"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_prepare_map_only_replaces_code_and_map() {
        // A prepare hook returning `{ code, map }` (no transform plugins, so
        // the skip path runs): the returned code becomes the output and
        // the OUTPUT map composes the hook map against the identity map
        // over the parse input (R6 prepare composition). The hook deletes the
        // original's first line: its map dst(0,0) -> src(1,0) says
        // "output line 0 came from original line 1".
        let map: SourceMap = hook_map(&[(0, 0, 1, 0)]);

        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(PrepareWithMapPlugin {
                code: String::from("let b = 7;"),
                map,
            })];

        let opts: CompileOptions = CompileOptions {
            file: "index.ts".to_string(),
            code: "const a = 1;\nlet b = 2;".to_string(),
            ..Default::default()
        };

        let out: CompileOutput = compile(opts, plugins).await.unwrap();

        // The hook's code replaced the source verbatim (no parse ran).
        assert_eq!(out.code, "let b = 7;");

        // The identity map over the parse input has one token (0,0)->
        // (0,0); composed through the hook map it resolves to the
        // ORIGINAL line 1.
        let token: oxc_sourcemap::Token =
            out.map.get_token(0).expect("one composed token");

        assert_eq!(token.get_dst_line(), 0);
        assert_eq!(token.get_dst_col(), 0);
        assert_eq!(token.get_src_line(), 1);
        assert_eq!(token.get_src_col(), 0);

        assert_eq!(out.map.get_source(0), Some("index.ts"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_prepare_map_composes_through_codegen_map() {
        // A prepare hook replaces the source AND provides its map; a transform
        // plugin runs, so the codegen map's src half refers to the
        // prepare source. The output map must resolve generated
        // positions back to the ORIGINAL code through the prepare map (R6:
        // the parse input maps back to the original source).
        let opts: CompileOptions = CompileOptions {
            file: "index.ts".to_string(),
            code: "console.log(1);".to_string(),
            ..Default::default()
        };

        // The prepare rewrite shifts the callee name by one column ("log" ->
        // "xog" is not valid; instead pad a comment line: dst line 1 =
        // src line 0). The hook map: dst(1, c) -> src(0, c).
        let map: SourceMap = hook_map(&[
            (1, 0, 0, 0),
            (1, 8, 0, 8),
            (1, 12, 0, 12),
            (1, 13, 0, 13),
            (1, 14, 0, 14),
        ]);

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(PrepareWithMapPlugin {
                code: String::from("// padded\nconsole.log(1);"),
                map,
            }),
            Plugin::new_shared(RenameCalleePlugin),
        ];

        let out: CompileOutput = compile(opts, plugins).await.unwrap();

        assert!(out.code.contains("consolex"), "{}", out.code);

        // The codegen map resolves into the prepare source (line 1);
        // composed through the hook map, generated positions must point
        // back at the ORIGINAL source line 0. Pick any token and check
        // its src line resolved to 0.
        let tokens: Vec<oxc_sourcemap::Token> = out.map.get_tokens().collect();

        assert!(!tokens.is_empty(), "mappings must be non-empty");

        for token in tokens {
            assert_eq!(
                token.get_src_line(),
                0,
                "generated line {} col {} must resolve to original line 0",
                token.get_dst_line(),
                token.get_dst_col(),
            );
        }

        assert_eq!(out.map.get_source(0), Some("index.ts"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_prepare_map_omitted_referenced_parse_input() {
        // A prepare hook replaces the source but omits the map: the output map
        // references the prepare rewritten source (the actual parse
        // input) — no composition ran. The identity map's source entry is
        // the file and every generated line maps 1:1 into the parse input.
        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(PrepareNoMapPlugin {
                code: String::from("let b = 2;\nlet c = 3;"),
            })];

        let opts: CompileOptions = CompileOptions {
            file: "index.ts".to_string(),
            code: "const a = 1;".to_string(),
            ..Default::default()
        };

        let out: CompileOutput = compile(opts, plugins).await.unwrap();

        assert_eq!(out.code, "let b = 2;\nlet c = 3;");

        // No transform plugin ran, so this is the identity map over the
        // prepare source: line 1 maps to line 1.
        let token: oxc_sourcemap::Token =
            out.map.get_token(1).expect("identity line token");

        assert_eq!(token.get_dst_line(), 1);
        assert_eq!(token.get_src_line(), 1);
        assert_eq!(token.get_src_col(), 0);

        assert_eq!(out.map.get_source(0), Some("index.ts"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_multi_prepare_chain_maps_compose_to_original() {
        // TWO prepare hooks, EACH returning `{ code, map }`: the whole chain
        // composes so the FINAL output's positions resolve back to the
        // ORIGINAL `options.code` (R6: compose the chain of prepare maps).
        //
        //   code_0 = "console.log(1);"          (line 0)
        //   hook 1  -> code_1 = "// one\nconsole.log(1);"
        //             map_1: dst(1, c) -> src(0, c)
        //   hook 2  -> code_2 = "// one\n// two\nconsole.log(1);"
        //             map_2: dst(2, c) -> src(1, c)
        //
        // Each hook also ASSERTS it received the previous hook's output
        // (hook 2 must see code_1, not code_0): the fold carries, so map_2
        // is genuinely relative to code_1. No transform plugin runs, so the
        // pipeline map is the identity map over code_2 (the parse input);
        // composed through map_2 then map_1, final line 2 must resolve to
        // ORIGINAL line 0. Carrying only the LAST map (map_2) resolves
        // line 2 to line 1 — code_1, which exists nowhere in the pipeline —
        // and a fixed-input fold hands hook 2 code_0, failing its probe.
        let map_1: SourceMap = hook_map(&[(1, 0, 0, 0), (1, 12, 0, 12)]);

        let map_2: SourceMap = hook_map(&[(2, 0, 1, 0), (2, 12, 1, 12)]);

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(CarryingPreparePlugin {
                expected_input: String::from("console.log(1);"),
                output: String::from("// one\nconsole.log(1);"),
                map: Some(map_1),
            }),
            Plugin::new_shared(CarryingPreparePlugin {
                expected_input: String::from("// one\nconsole.log(1);"),
                output: String::from("// one\n// two\nconsole.log(1);"),
                map: Some(map_2),
            }),
        ];

        let opts: CompileOptions = CompileOptions {
            file: "index.ts".to_string(),
            code: "console.log(1);".to_string(),
            ..Default::default()
        };

        let out: CompileOutput = compile(opts, plugins).await.unwrap();

        assert_eq!(out.code, "// one\n// two\nconsole.log(1);");

        let tokens: Vec<oxc_sourcemap::Token> = out.map.get_tokens().collect();

        assert!(!tokens.is_empty(), "mappings must be non-empty");

        for token in tokens {
            assert_eq!(
                token.get_src_line(),
                0,
                "final ({},{}) must resolve to original line 0 through BOTH chain maps",
                token.get_dst_line(),
                token.get_dst_col(),
            );
        }

        assert_eq!(out.map.get_source(0), Some("index.ts"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_prepare_fold_carries_between_hooks() {
        // Two prepare plugins: the first returns map-less code; the second records
        // what it received. The carried fold must hand it the first hook's
        // rewrite, not the original source.
        let seen: std::sync::Arc<std::sync::Mutex<Vec<String>>> =
            std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(PrepareNoMapPlugin {
                code: String::from("let a = 1;"),
            }),
            Plugin::new_shared(PrepareProbePlugin {
                seen: std::sync::Arc::clone(&seen),
            }),
        ];

        let opts: CompileOptions = CompileOptions {
            file: "index.ts".to_string(),
            code: "const original = 1;".to_string(),
            ..Default::default()
        };

        let out: CompileOutput = compile(opts, plugins).await.unwrap();

        assert_eq!(
            seen.lock().unwrap().clone(),
            vec![String::from("let a = 1;")],
            "the second prepare hook must see the first hook's rewrite",
        );

        assert_eq!(out.code, "let a = 1;");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_finalize_fold_carries_between_hooks() {
        // Two finalize plugins: the first rewrites the generated code; the second
        // records what it received. The carried fold must hand it the first
        // hook's rewrite, not the generated code.
        let seen: std::sync::Arc<std::sync::Mutex<Vec<String>>> =
            std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(NoopTransformPlugin),
            Plugin::new_shared(FinalizeNoMapPlugin {
                code: String::from("const rewritten = 2;"),
            }),
            Plugin::new_shared(FinalizeProbePlugin {
                seen: std::sync::Arc::clone(&seen),
            }),
        ];

        let opts: CompileOptions = CompileOptions {
            file: "index.ts".to_string(),
            code: "const a = 1;".to_string(),
            ..Default::default()
        };

        let out: CompileOutput = compile(opts, plugins).await.unwrap();

        assert_eq!(
            seen.lock().unwrap().clone(),
            vec![String::from("const rewritten = 2;")],
            "the second finalize hook must see the first hook's rewrite",
        );

        assert_eq!(out.code, "const rewritten = 2;");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_multi_finalize_chain_maps_compose() {
        // TWO finalize hooks, EACH returning `{ code, map }` over its own edit:
        // the whole chain composes so the FINAL output's positions resolve
        // through BOTH finalize stages and then the pipeline map back to the
        // ORIGINAL source (final -> code_1 -> generated -> original).
        //
        //   generated = "console.log(1);"        (codegen line 0)
        //   finalize 1 -> out_1 = "// one\nconsole.log(1);"
        //             map_1: dst(1, c) -> src(0, c)  (src = generated line 0)
        //   finalize 2 -> out_2 = "// one\n// two\nconsole.log(1);"
        //             map_2: dst(2, c) -> src(1, c)  (src = code_1 line 1)
        //
        // The pipeline map's dst space is the generated line 0; finalize 1's
        // map src space is also generated line 0. Composing the LAST-served
        // map (map_2) as upstream and (map_1, pipeline) as incrementals
        // resolves the final line 2 back to ORIGINAL line 0. An inverted
        // direction (pipeline upstream) would drop map_2's dst tokens; a
        // fixed-input fold would fail hook 2's carry assertion.
        let map_1: SourceMap = hook_map(&[
            (1, 0, 0, 0),
            (1, 9, 0, 9),
            (1, 13, 0, 13),
            (1, 14, 0, 14),
        ]);

        let map_2: SourceMap = hook_map(&[
            (2, 0, 1, 0),
            (2, 9, 1, 9),
            (2, 13, 1, 13),
            (2, 14, 1, 14),
        ]);

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(NoopTransformPlugin),
            Plugin::new_shared(CarryingFinalizePlugin {
                expected_input: String::from("console.log(1);"),
                output: String::from("// one\nconsole.log(1);"),
                map: Some(map_1),
            }),
            Plugin::new_shared(CarryingFinalizePlugin {
                expected_input: String::from("// one\nconsole.log(1);"),
                output: String::from("// one\n// two\nconsole.log(1);"),
                map: Some(map_2),
            }),
        ];

        let opts: CompileOptions = CompileOptions {
            file: "index.ts".to_string(),
            code: "console.log(1);".to_string(),
            ..Default::default()
        };

        let out: CompileOutput = compile(opts, plugins).await.unwrap();

        assert_eq!(out.code, "// one\n// two\nconsole.log(1);");

        // The composed map keeps the POST chain's dst tokens (the final
        // output positions), not the codegen map's.
        assert_eq!(out.map.get_source(0), Some("index.ts"));

        let tokens: Vec<oxc_sourcemap::Token> = out.map.get_tokens().collect();

        assert!(
            tokens
                .iter()
                .any(|token: &oxc_sourcemap::Token| token.get_dst_line() == 2),
            "the final-output (line 2) dst tokens must survive the composition",
        );

        // The final line 2 statement came from original line 0 through
        // BOTH finalize maps and the codegen map; every line-2 token must
        // resolve to original line 0.
        for token in tokens
            .iter()
            .filter(|token: &&oxc_sourcemap::Token| token.get_dst_line() == 2)
        {
            assert_eq!(
                token.get_src_line(),
                0,
                "final line 2 dst ({},{}) must resolve to original line 0",
                token.get_dst_line(),
                token.get_dst_col(),
            );
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_prepare_omission_mid_chain_references_parse_input() {
        // Hook 1 returns `{ code, map }`, hook 2 returns `{ code }` with
        // the map OMITTED: the composition chain breaks at the omission
        // (per R6, no composition across the gap), so the final map
        // references the prepare rewritten source (the actual parse
        // input). The identity map's line 1 must map to itself, NOT to
        // an original position resolved through hook 1's map.
        let map_1: SourceMap = hook_map(&[(1, 0, 0, 0)]);

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(PrepareWithMapPlugin {
                code: String::from("// one\nconsole.log(1);"),
                map: map_1,
            }),
            Plugin::new_shared(PrepareNoMapPlugin {
                code: String::from("// rewritten\nlet b = 2;"),
            }),
        ];

        let opts: CompileOptions = CompileOptions {
            file: "index.ts".to_string(),
            code: "const a = 1;".to_string(),
            ..Default::default()
        };

        let out: CompileOutput = compile(opts, plugins).await.unwrap();

        assert_eq!(out.code, "// rewritten\nlet b = 2;");

        // The identity map over the parse input ran UN-composed: line 1
        // maps to line 1 (hook 1's map would have resolved it to 0).
        let token: oxc_sourcemap::Token =
            out.map.get_token(1).expect("identity line token");

        assert_eq!(token.get_dst_line(), 1);

        assert_eq!(token.get_src_line(), 1);

        assert_eq!(token.get_src_col(), 0);

        assert_eq!(out.map.get_source(0), Some("index.ts"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_finalize_map_composes_with_codegen_map() {
        // A finalize hook returns `{ code, map }`: the map is incremental
        // relative to the GENERATED code (dst positions in the final
        // output, src positions in the generated code); the output map
        // must be the POST map's dst tokens composing to the ORIGINAL
        // source through the codegen map (R6: finalize's map composes with
        // the codegen map as final -> generated -> original).
        //
        // The probe scenario: original line 0 holds `console.log(1)` and
        // the codegen rename makes it `consolex.log(1)`; the finalize edit
        // renames it back to `console` but rewrites the `1` to a `2`,
        // leaving TWO identical `console.log(2);` lines in the final
        // code. ORIGINAL line 0's content therefore survives only at
        // final line 1, and the composed map must resolve final line 1
        // (F = 1) back to original line 0 (L = 0) through BOTH maps.
        let opts: CompileOptions = CompileOptions {
            file: "index.ts".to_string(),
            code: "console.log(1);".to_string(),
            ..Default::default()
        };

        // The finalize map over the finalize hook's own edit: line 0 is the
        // fresh banner (dst(0, 0) -> src(0, 0) — the generated first
        // line), line 1 holds the rewritten statement (dst(1, 9/13/14)
        // -> src(0, 9/13/14) — the generate generated columns).
        let map: SourceMap = hook_map(&[
            (0, 0, 0, 0),
            (1, 0, 0, 0),
            (1, 9, 0, 9),
            (1, 13, 0, 13),
            (1, 14, 0, 14),
        ]);

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(RenameCalleePlugin),
            Plugin::new_shared(FinalizeWithMapPlugin {
                code: String::from("// banner\nconsole.log(2);"),
                map,
            }),
        ];

        let out: CompileOutput = compile(opts, plugins).await.unwrap();

        // The hook's code replaced the generated code verbatim, banner
        // and all (only the trailing newline strips).
        assert_eq!(out.code, "// banner\nconsole.log(2);");

        // The composed map carries the CODEGEN map's source (the
        // original `index.ts` contents), not the finalize hook's.
        assert_eq!(out.map.get_source(0), Some("index.ts"));

        let tokens: Vec<oxc_sourcemap::Token> = out.map.get_tokens().collect();

        // The POST map's dst positions are the only output positions:
        // 5 composed tokens, one per finalize-map token.
        assert_eq!(tokens.len(), 5, "finalize dst tokens are kept");

        // Line 0 of the finalize edit is a fully fresh banner line; its
        // head token resolves through the pipeline map's head rule to
        // nothing (src stays 0/0), never to the original's line 0
        // columns — the banner does not come from the original.
        let banner: oxc_sourcemap::Token = tokens[0];

        assert_eq!((banner.get_dst_line(), banner.get_dst_col()), (0, 0));

        assert_eq!(
            (banner.get_src_line(), banner.get_src_col()),
            (0, 0),
            "the banner is fresh text, not original content",
        );

        // Final line 1 carries the L content. Each finalize token's dst
        // (1, c) maps to the generated column c, and the codegen map
        // resolves generated col 9 -> original col 8, col 13 -> 12,
        // col 14 -> 13 (the rename shrinks by one). These assertions
        // traverse BOTH maps (finalize's dst -> dst-of-codegen, then the
        // codegen's src); the un-composed map resolves them to the
        // GENERATED positions instead and fails.
        let expected: &[(u32, u32, u32, u32)] =
            &[(1, 0, 0, 0), (1, 9, 0, 8), (1, 13, 0, 12), (1, 14, 0, 13)];

        assert_eq!(tokens[1].get_dst_line(), 1);

        for (idx, &(dst_line, dst_col, src_line, src_col)) in
            expected.iter().enumerate()
        {
            let token: oxc_sourcemap::Token = tokens[idx + 1];

            assert_eq!(
                (token.get_dst_line(), token.get_dst_col()),
                (dst_line, dst_col),
                "finalize dst position must be preserved verbatim",
            );

            assert_eq!(
                (token.get_src_line(), token.get_src_col()),
                (src_line, src_col),
                "final ({},{}) must resolve to original ({},{}) through both maps",
                dst_line,
                dst_col,
                src_line,
                src_col,
            );
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_finalize_map_omitted_keeps_codegen_map() {
        // A finalize hook replaces the code but omits the map: NO composition,
        // the codegen map stays the output map unchanged.
        let opts: CompileOptions = CompileOptions {
            file: "index.ts".to_string(),
            code: "console.log(1);".to_string(),
            ..Default::default()
        };

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(NoopTransformPlugin),
            Plugin::new_shared(FinalizeNoMapPlugin {
                code: String::from("console.log(2);"),
            }),
        ];

        let out: CompileOutput = compile(opts, plugins).await.unwrap();

        assert!(out.code.contains("console.log(2);"), "{}", out.code);

        // The codegen map over the ORIGINAL parse input is untouched:
        // source and token shape match the plain codegen map.
        assert_eq!(out.map.get_source(0), Some("index.ts"));

        let token: oxc_sourcemap::Token =
            out.map.get_token(0).expect("codegen token");

        assert_eq!(token.get_src_line(), 0);
    }

    // A minimal compile_start observer recording the resolved options; used
    // by the duplicate-names e2e test.
    #[derive(Debug)]
    struct CompileStartObserverPlugin {
        name: &'static str,
        observer: std::sync::Arc<std::sync::Mutex<Vec<ResolvedOptions>>>,
    }

    impl Plugin for CompileStartObserverPlugin {
        fn name(&self) -> Cow<'static, str> {
            self.name.into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::CompileStart
        }

        fn compile_start<'a>(
            &'a self,
            _ctx: &'a telarel_plugin::PluginContext<'_>,
            args: &'a telarel_plugin::CompileStartArgs,
        ) -> impl Future<Output = telarel_plugin::NotifyReturn> + Send {
            let options: ResolvedOptions = args.options.clone();

            let observer: std::sync::Arc<
                std::sync::Mutex<Vec<ResolvedOptions>>,
            > = std::sync::Arc::clone(&self.observer);

            async move {
                observer.lock().unwrap().push(options);

                Ok(())
            }
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_duplicate_names_reach_resolved_options() {
        // Duplicate plugin names are allowed everywhere (no dedup):
        // two plugins with the same name both run, and ResolvedOptions
        // .plugins carries BOTH names in settled order.
        let observer: std::sync::Arc<std::sync::Mutex<Vec<ResolvedOptions>>> =
            std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(CompileStartObserverPlugin {
                name: "twin",
                observer: std::sync::Arc::clone(&observer),
            }),
            Plugin::new_shared(CompileStartObserverPlugin {
                name: "twin",
                observer: std::sync::Arc::clone(&observer),
            }),
        ];

        let opts: CompileOptions = CompileOptions {
            file: "index.ts".to_string(),
            code: "const a = 1;".to_string(),
            ..Default::default()
        };

        compile(opts, plugins).await.unwrap();

        let recorded: Vec<ResolvedOptions> = observer.lock().unwrap().clone();

        assert_eq!(recorded.len(), 2, "each twin's compile_start ran");

        assert_eq!(recorded[0].plugins, vec![String::from("twin"); 2]);

        assert_eq!(recorded[1].plugins, vec![String::from("twin"); 2]);
    }

    // A transform hook that always errors; used to verify the
    // transform-error compile_end path (the observer records compile_end's
    // payload on that path).
    #[derive(Debug)]
    struct FailingTransformHookPlugin;

    impl Plugin for FailingTransformHookPlugin {
        fn name(&self) -> Cow<'static, str> {
            "failing-transform-hook".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Transform
        }

        fn transform<'a, 'ast: 'a>(
            &'a self,
            _ctx: &'a telarel_plugin::PluginContext<'_>,
            _args: TransformArgs<'ast>,
        ) -> impl Future<Output = TransformReturn<'ast>> + 'a {
            async move { Err(anyhow::anyhow!("transform boom")) }
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_end_runs_on_transform_error_with_prepare_map_last_good()
     {
        // A prepare hook returned a map, then the transform hook errors:
        // the error-path compile_end carries the last good state = the
        // prepare code with the prepare map (un-composed — composition
        // applies to the OUTPUT map only).
        let seen: std::sync::Arc<std::sync::Mutex<Vec<CompileEndShape>>> =
            std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));

        let prepare_map: SourceMap = hook_map(&[(0, 0, 0, 0), (0, 7, 0, 5)]);

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(PrepareWithMapPlugin {
                code: String::from("const boom = 1;"),
                map: prepare_map,
            }),
            Plugin::new_shared(ObserveCompileEndPlugin {
                seen: std::sync::Arc::clone(&seen),
            }),
            Plugin::new_shared(FailingTransformHookPlugin),
        ];

        let error: CompileError =
            compile(options(), plugins).await.unwrap_err();

        let message: String = error.to_string();

        assert!(message.contains("transform hook:"), "{}", message);

        let recorded: Vec<CompileEndShape> = seen.lock().unwrap().clone();

        assert_eq!(recorded.len(), 1, "compile_end ran on the transform error");

        assert_eq!(
            recorded[0].code, "const boom = 1;",
            "last good code is the prepare parse input",
        );

        assert!(
            recorded[0].has_map,
            "the prepare map is part of the last good state",
        );

        assert!(
            recorded[0]
                .err
                .as_deref()
                .is_some_and(|err| err.contains("transform boom")),
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_end_finalize_error_last_good_map_is_codegen_map() {
        // A finalize hook errors after codegen: the last good state carries
        // the GENERATED code and the codegen map.
        let seen: std::sync::Arc<std::sync::Mutex<Vec<CompileEndShape>>> =
            std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));

        let opts: CompileOptions = CompileOptions {
            file: "index.ts".to_string(),
            code: "console.log(1);".to_string(),
            ..Default::default()
        };

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(NoopTransformPlugin),
            Plugin::new_shared(ObserveCompileEndPlugin {
                seen: std::sync::Arc::clone(&seen),
            }),
            Plugin::new_shared(FailingFinalizePlugin),
        ];

        let error: CompileError = compile(opts, plugins).await.unwrap_err();

        let message: String = error.to_string();

        assert!(message.contains("finalize hook"), "{}", message);

        let recorded: Vec<CompileEndShape> = seen.lock().unwrap().clone();

        assert_eq!(recorded.len(), 1, "compile_end ran on the finalize error");

        // The last good code is the GENERATED code (trailing newline
        // stripped exactly as the finalize hook would have received it).
        assert!(
            recorded[0].code.contains("console.log(1);"),
            "{}",
            recorded[0].code
        );

        assert!(
            recorded[0].has_map,
            "the codegen map is part of the last good state",
        );

        assert!(
            recorded[0]
                .err
                .as_deref()
                .is_some_and(|err| err.contains("finalize hook")),
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_end_parse_error_last_good_carries_prepare_map() {
        // A prepare hook returned a map, then the parse fails: the
        // error-path compile_end carries the prepare code and the pre
        // map (the parse-error last-good carry).
        let seen: std::sync::Arc<std::sync::Mutex<Vec<CompileEndShape>>> =
            std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));

        let prepare_map: SourceMap = hook_map(&[(0, 0, 0, 0), (0, 5, 0, 3)]);

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(PrepareWithMapPlugin {
                code: String::from("const = ;"),
                map: prepare_map,
            }),
            Plugin::new_shared(ObserveCompileEndPlugin {
                seen: std::sync::Arc::clone(&seen),
            }),
            Plugin::new_shared(NoopTransformPlugin),
        ];

        let error: CompileError =
            compile(options(), plugins).await.unwrap_err();

        let message: String = error.to_string();

        assert!(message.contains("index.ts"), "{}", message);

        let recorded: Vec<CompileEndShape> = seen.lock().unwrap().clone();

        assert_eq!(recorded.len(), 1, "compile_end ran on the parse error");

        assert_eq!(recorded[0].code, "const = ;");

        assert!(
            recorded[0].has_map,
            "prepare map carried as the last good map"
        );

        assert!(recorded[0].err.is_some());
    }
}
