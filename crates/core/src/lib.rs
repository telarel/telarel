//! # Telarel Core
//!
//! A pipeline orchestrator for the compiler.
//!
//! This crate drives the per-file compile pipeline: it runs plugin hooks,
//! parses the source, applies transforms, and produces code with a map.

use oxc::allocator::Allocator;
use oxc::ast::ast::Program;
use oxc_sourcemap::SourceMap as OxcSourceMap;
use oxc_sourcemap::SourceMapBuilder;

use telarel_common::{
    CompileContext, CompileError, HookUsage, ParseOptions, parse,
};
use telarel_plugin::__internal::PluginDriver;
use telarel_plugin::{PostArgs, PreArgs, SharedPluginable, TransformArgs};

pub use telarel_common::CompileOptions;

/// An owned source map produced by a compile run.
pub type SourceMap = OxcSourceMap<'static>;

/// Strip a trailing `\n`, then a trailing `\r`, so `\n`, `\r\n`, and
/// lone `\r` terminators all disappear, but still keeping interior
/// line endings survive.
///
/// The input is owned, so the trailing terminator is removed in place
/// instead of copying the whole string into a fresh `String`.
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

/// Result of a successful compile.
#[derive(Debug, Clone)]
pub struct CompileOutput {
    /// Generated code.
    pub code: String,
    /// Source map.
    pub map: SourceMap,
}

/// Run the per-file pipeline over `options` with `plugins`.
pub async fn compile(
    options: CompileOptions,
    plugins: Vec<SharedPluginable>,
) -> Result<CompileOutput, CompileError> {
    let driver: PluginDriver = PluginDriver::new(plugins);

    let resolved: CompileOptions =
        driver.options(options).await.map_err(|error| {
            CompileError::from_message(&format!("options hook: {error:#}"))
        })?;

    let ctx: CompileContext<'_> =
        CompileContext::new(&resolved.cwd, &resolved.file, &resolved.code);

    driver
        .pre(&ctx, &PreArgs { file: &resolved.file, code: &resolved.code })
        .await
        .map_err(|error| {
            CompileError::from_message(&format!("pre hook: {error:#}"))
        })?;

    let (code, map): (String, SourceMap) = if driver
        .usage()
        .contains(HookUsage::Transform)
    {
        // Parse ONCE before the transform chain. Root the resolved source in
        // the allocator so the parsed program borrows from the same
        // allocation.
        let allocator: Allocator = Allocator::default();

        let source: &'_ str = allocator.alloc_str(&resolved.code);

        let parse_options: ParseOptions<'_, '_> = ParseOptions {
            context: &ctx,
            allocator: &allocator,
            file: &resolved.file,
            code: source,
        };

        let original: Program<'_> = parse(parse_options)?.program;

        let transform_args: TransformArgs<'_> = TransformArgs {
            allocator: &allocator,
            file: &resolved.file,
            program: &original,
        };

        let replaced: Option<&Program<'_>> =
            driver.transform(&ctx, &transform_args).await.map_err(|error| {
                CompileError::from_message(&format!(
                    "transform hook: {error:#}"
                ))
            })?;

        // The driver returns a borrow into the compile allocator; the core
        // only borrows for codegen and never copies the program.
        let current: &Program<'_> = replaced.unwrap_or(&original);

        // Codegen from the AST (original when no plugin replaced).
        let result: telarel_common::CodegenResult<'_> =
            telarel_common::codegen(telarel_common::CodegenOptions {
                file: &resolved.file,
                program: current,
            });

        let code: String = strip_trailing_newline(result.code);

        let map: SourceMap = result.map.into_owned();

        (code, map)
    } else {
        // No plugin uses `transform`: skip parse and codegen entirely and
        // pass the source through with a per-line identity map. A clone is
        // required here: `ctx` borrows `resolved.code` until the `post` hook
        // below, so the strip cannot consume `resolved.code` in place.
        let code: String = strip_trailing_newline(resolved.code.clone());

        let map: SourceMap = identity_map(&resolved.file, &resolved.code);

        (code, map)
    };

    driver
        .post(&ctx, &PostArgs { file: &resolved.file, code: &code })
        .await
        .map_err(|error| {
            CompileError::from_message(&format!("post hook: {error:#}"))
        })?;

    Ok(CompileOutput { code, map })
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;
    use std::sync::Arc;

    use telarel_common::CompileContext;
    use telarel_common::CompileOptions;
    use telarel_common::PartialCompileOptions;
    use telarel_plugin::{
        Plugin, SharedPluginable, TransformArgs, TransformOutput,
        TransformReturn,
    };

    use oxc::allocator::{ArenaBox, ArenaVec, CloneIn, GetAllocator};
    use oxc::ast::ast::{
        Argument, CallExpression, Directive, Expression, ExpressionStatement,
        IdentifierReference, NumericLiteral, Program, Statement,
        StaticMemberExpression, StringLiteral,
    };
    use oxc::ast::builder::AstBuilder;
    use oxc::span::SPAN;
    use oxc::str::Ident;

    use super::*;

    #[derive(Debug)]
    struct NoopPlugin;

    impl Plugin for NoopPlugin {
        fn name(&self) -> Cow<'static, str> {
            "noop".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Pre
        }
    }

    const TARGET: &str = "console";

    const REPLACEMENT: &str = "consolex";

    const DIRECTIVE: &str = "x-telarel-chain";

    #[derive(Debug)]
    struct RenameCalleePlugin;

    impl Plugin for RenameCalleePlugin {
        fn name(&self) -> Cow<'static, str> {
            "rename-callee".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Transform
        }

        // Rebuild the Program with every `console` Identifier renamed to
        // `consolex` (AstBuilder walk over the allocator; build the replacement
        // in `args.allocator`). Returns Some(TransformOutput { program }).
        async fn transform<'a>(
            &'a self,
            _ctx: &'a CompileContext<'a>,
            args: &'a TransformArgs<'a>,
        ) -> TransformReturn<'a> {
            let builder: AstBuilder<'a> = AstBuilder::new(args.allocator);
            let original: &'a Program<'a> = args.program;

            let body: ArenaVec<'a, Statement<'a>> = ArenaVec::from_iter_in(
                original.body.iter().map(|statement: &Statement<'a>| {
                    rename_statement(statement, &builder)
                }),
                &builder,
            );
            let directives: ArenaVec<'a, Directive<'a>> =
                ArenaVec::from_iter_in(
                    original.directives.iter().map(
                        |directive: &Directive<'a>| {
                            directive.clone_in(builder.allocator())
                        },
                    ),
                    &builder,
                );

            let program: Program<'a> = Program::new(
                original.span,
                original.source_type,
                original.source_text,
                original.comments.clone_in(builder.allocator()),
                original.hashbang.clone_in(builder.allocator()),
                directives,
                body,
                &builder,
            );

            Ok(Some(TransformOutput { program }))
        }
    }

    fn rename_statement<'a>(
        statement: &Statement<'a>,
        builder: &AstBuilder<'a>,
    ) -> Statement<'a> {
        match statement {
            | Statement::ExpressionStatement(expression_statement) => {
                let expression: Expression<'a> = rename_expression(
                    &expression_statement.expression,
                    builder,
                );
                let node: ExpressionStatement<'a> = ExpressionStatement::new(
                    expression_statement.span,
                    expression,
                    builder,
                );
                Statement::ExpressionStatement(ArenaBox::new_in(node, builder))
            },
            | other => other.clone_in(builder.allocator()),
        }
    }

    fn rename_expression<'a>(
        expression: &Expression<'a>,
        builder: &AstBuilder<'a>,
    ) -> Expression<'a> {
        match expression {
            | Expression::Identifier(identifier) => {
                let name: &str = identifier.name.as_str();
                let renamed: &str =
                    if name == TARGET { REPLACEMENT } else { name };
                let ident: Ident<'a> = Ident::from_str_in(renamed, builder);
                let node: IdentifierReference<'a> =
                    IdentifierReference::new(identifier.span, ident, builder);
                Expression::Identifier(ArenaBox::new_in(node, builder))
            },
            | Expression::CallExpression(call) => {
                let callee: Expression<'a> =
                    rename_expression(&call.callee, builder);
                let arguments: ArenaVec<'a, Argument<'a>> =
                    ArenaVec::from_iter_in(
                        call.arguments.iter().map(|argument: &Argument<'a>| {
                            rename_argument(argument, builder)
                        }),
                        builder,
                    );
                let node: CallExpression<'a> = CallExpression::new(
                    call.span,
                    callee,
                    None,
                    arguments,
                    call.optional,
                    builder,
                );
                Expression::CallExpression(ArenaBox::new_in(node, builder))
            },
            | Expression::StaticMemberExpression(member) => {
                let object: Expression<'a> =
                    rename_expression(&member.object, builder);
                let node: StaticMemberExpression<'a> =
                    StaticMemberExpression::new(
                        member.span,
                        object,
                        member.property.clone_in(builder.allocator()),
                        member.optional,
                        builder,
                    );
                Expression::StaticMemberExpression(ArenaBox::new_in(
                    node, builder,
                ))
            },
            | Expression::NumericLiteral(literal) => {
                let node: NumericLiteral<'a> = NumericLiteral::new(
                    literal.span,
                    literal.value,
                    literal.raw.clone_in(builder.allocator()),
                    literal.base,
                    builder,
                );
                Expression::NumericLiteral(ArenaBox::new_in(node, builder))
            },
            | other => other.clone_in(builder.allocator()),
        }
    }

    fn rename_argument<'a>(
        argument: &Argument<'a>,
        builder: &AstBuilder<'a>,
    ) -> Argument<'a> {
        match argument.as_expression() {
            | Some(expression) => {
                Argument::from(rename_expression(expression, builder))
            },
            | None => argument.clone_in(builder.allocator()),
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

        // Rebuild the Program with one extra directive appended (AstBuilder
        // walk over the allocator; build the replacement in `args.allocator`).
        // Returns Some(TransformOutput { program }).
        async fn transform<'a>(
            &'a self,
            _ctx: &'a CompileContext<'a>,
            args: &'a TransformArgs<'a>,
        ) -> TransformReturn<'a> {
            let builder: AstBuilder<'a> = AstBuilder::new(args.allocator);
            let original: &'a Program<'a> = args.program;

            let string_literal: StringLiteral<'a> =
                StringLiteral::new(SPAN, DIRECTIVE, None, &builder);

            let directive: Directive<'a> =
                Directive::new(SPAN, string_literal, DIRECTIVE, &builder);

            let mut directives: ArenaVec<'a, Directive<'a>> =
                original.directives.clone_in(builder.allocator());

            directives.push(directive);

            let body: ArenaVec<'a, Statement<'a>> =
                original.body.clone_in(builder.allocator());

            let program: Program<'a> = Program::new(
                original.span,
                original.source_type,
                original.source_text,
                original.comments.clone_in(builder.allocator()),
                original.hashbang.clone_in(builder.allocator()),
                directives,
                body,
                &builder,
            );

            Ok(Some(TransformOutput { program }))
        }
    }

    #[derive(Debug)]
    struct NoneReturningTransformPlugin;

    impl Plugin for NoneReturningTransformPlugin {
        fn name(&self) -> Cow<'static, str> {
            "none-returning-transform".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Transform
        }

        // A transform hook that never replaces the program: Ok(None) must
        // leave the current program in place for the rest of the chain.
        async fn transform<'a>(
            &'a self,
            _ctx: &'a CompileContext<'a>,
            _args: &'a TransformArgs<'a>,
        ) -> TransformReturn<'a> {
            Ok(None)
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

        async fn options(
            &self,
            _options: &CompileOptions,
        ) -> anyhow::Result<Option<PartialCompileOptions>> {
            Ok(Some(PartialCompileOptions {
                code: Some("const rewritten = 7;".to_string()),
                ..PartialCompileOptions::default()
            }))
        }
    }

    #[derive(Debug)]
    struct FailingPrePlugin;

    impl Plugin for FailingPrePlugin {
        fn name(&self) -> Cow<'static, str> {
            "failing-pre".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Pre
        }

        async fn pre(
            &self,
            _ctx: &'_ CompileContext<'_>,
            _args: &'_ telarel_plugin::PreArgs<'_>,
        ) -> anyhow::Result<()> {
            Err(anyhow::anyhow!("stage boom"))
        }
    }

    #[derive(Debug)]
    struct FailingPostPlugin;

    impl Plugin for FailingPostPlugin {
        fn name(&self) -> Cow<'static, str> {
            "failing-post".into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Post
        }

        async fn post(
            &self,
            _ctx: &'_ CompileContext<'_>,
            _args: &'_ telarel_plugin::PostArgs<'_>,
        ) -> anyhow::Result<()> {
            Err(anyhow::anyhow!("stage boom"))
        }
    }

    fn options() -> CompileOptions {
        CompileOptions {
            cwd: "/repo".to_string(),
            file: "index.ts".to_string(),
            code: "const a = 1;".to_string(),
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
            cwd: "/repo".to_string(),
            file: "index.ts".to_string(),
            code: "console.log(1);".to_string(),
        };

        let plugins: Vec<SharedPluginable> = vec![Arc::new(NoopPlugin)];

        let out = compile(opts, plugins).await.unwrap();

        assert_eq!(out.code, "console.log(1);");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_strips_trailing_newline() {
        let opts: CompileOptions = CompileOptions {
            cwd: "/repo".to_string(),
            file: "index.ts".to_string(),
            code: "const a = 1;\n".to_string(),
        };

        let out: crate::CompileOutput = compile(opts, vec![]).await.unwrap();

        assert_eq!(out.code, "const a = 1;");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_strips_crlf_trailing_newline() {
        // CRLF sources must not leave a stray `\r` after the `\n` strip.
        let opts: CompileOptions = CompileOptions {
            cwd: "/repo".to_string(),
            file: "index.ts".to_string(),
            code: "const a = 1;\r\n".to_string(),
        };

        let out: crate::CompileOutput = compile(opts, vec![]).await.unwrap();

        assert_eq!(out.code, "const a = 1;");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_preserves_interior_crlf() {
        // Only the trailing terminator is stripped; interior CRLF is intact.
        let opts: CompileOptions = CompileOptions {
            cwd: "/repo".to_string(),
            file: "index.ts".to_string(),
            code: "const a = 1;\r\nconst b = 2;".to_string(),
        };

        let out: crate::CompileOutput = compile(opts, vec![]).await.unwrap();

        assert_eq!(out.code, "const a = 1;\r\nconst b = 2;");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_strips_lone_cr_trailing_newline() {
        // A lone `\r` terminator is also stripped, matching the documented
        // behavior of the trim (`\n`, `\r\n`, and lone `\r`).
        let opts: CompileOptions = CompileOptions {
            cwd: "/repo".to_string(),
            file: "index.ts".to_string(),
            code: "const a = 1;\r".to_string(),
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
        // RenameCalleePlugin over `console.log(1);` must yield `consolex`.
        let opts: CompileOptions = CompileOptions {
            cwd: "/repo".to_string(),
            file: "index.ts".to_string(),
            code: "console.log(1);".to_string(),
        };

        let plugins: Vec<SharedPluginable> = vec![Arc::new(RenameCalleePlugin)];

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
            cwd: "/repo".to_string(),
            file: "index.ts".to_string(),
            code: "console.log(1);".to_string(),
        };

        let out =
            compile(opts, vec![Arc::new(RenameCalleePlugin)]).await.unwrap();

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
            cwd: "/repo".to_string(),
            file: "index.ts".to_string(),
            code: "const = ;".to_string(),
        };

        let error: CompileError =
            compile(opts, vec![Arc::new(RenameCalleePlugin)])
                .await
                .unwrap_err();

        assert!(error.to_string().contains("index.ts"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_no_transform_usage_skips_parse() {
        // Invalid syntax passes through unparsed when no plugin uses
        // `transform` — the skip path never parses.
        let opts: CompileOptions = CompileOptions {
            cwd: "/repo".to_string(),
            file: "index.ts".to_string(),
            code: "const = ;".to_string(),
        };

        let out: crate::CompileOutput = compile(opts, vec![]).await.unwrap();

        assert_eq!(out.code, "const = ;");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_no_transform_usage_identity_map() {
        let opts: CompileOptions = CompileOptions {
            cwd: "/repo".to_string(),
            file: "index.ts".to_string(),
            code: "const a = 1;\nconst b = 2;\n".to_string(),
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
    async fn test_compile_noop_pre_plugin_does_not_parse() {
        // A plugin declaring only PRE must not force the parse path; output
        // stays verbatim (proves NoopPlugin's PRE declaration doesn't leak
        // into the transform decision).
        let opts: CompileOptions = CompileOptions {
            cwd: "/repo".to_string(),
            file: "index.ts".to_string(),
            code: "console.log(1);\n".to_string(),
        };

        let plugins: Vec<SharedPluginable> = vec![Arc::new(NoopPlugin)];

        let out = compile(opts, plugins).await.unwrap();

        assert_eq!(out.code, "console.log(1);");
        assert_eq!(out.map.get_source(0), Some("index.ts"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_options_hook_rewrites_code() {
        let plugins: Vec<SharedPluginable> =
            vec![Arc::new(RewriteCodeOptionsPlugin)];

        let out: crate::CompileOutput =
            compile(options(), plugins).await.unwrap();

        assert!(out.code.contains("rewritten"), "{}", out.code);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_pre_hook_error_aborts() {
        let plugins: Vec<SharedPluginable> = vec![Arc::new(FailingPrePlugin)];

        let error: CompileError =
            compile(options(), plugins).await.unwrap_err();

        let message: String = error.to_string();

        assert!(message.contains("pre hook"), "{}", message);
        assert!(message.contains("stage boom"), "{}", message);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_output_map_is_alias() {
        // `SourceMap` in `CompileOutput` must be the owned-map alias: assign it
        // to an explicitly-aliased variable to prove the types agree.
        let opts: CompileOptions = CompileOptions {
            cwd: "/repo".to_string(),
            file: "index.ts".to_string(),
            code: "const a = 1;".to_string(),
        };

        let out: crate::CompileOutput = compile(opts, vec![]).await.unwrap();

        let map: SourceMap = out.map;

        assert_eq!(map.get_source(0), Some("index.ts"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_post_hook_error_aborts() {
        let plugins: Vec<SharedPluginable> = vec![Arc::new(FailingPostPlugin)];

        let error: CompileError =
            compile(options(), plugins).await.unwrap_err();

        let message: String = error.to_string();

        assert!(message.contains("post hook"), "{}", message);
        assert!(message.contains("stage boom"), "{}", message);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_transform_chain_multiple_replacements() {
        // Two replacing plugins in sequence: the rename must survive into the
        // second plugin and both transformations must appear in the final
        // code (proves the chain carries each replacement by reference).
        let opts: CompileOptions = CompileOptions {
            cwd: "/repo".to_string(),
            file: "index.ts".to_string(),
            code: "console.log(1);".to_string(),
        };

        let plugins: Vec<SharedPluginable> =
            vec![Arc::new(RenameCalleePlugin), Arc::new(AppendDirectivePlugin)];

        let out = compile(opts, plugins).await.unwrap();

        assert!(out.code.contains("consolex"), "{}", out.code);
        assert!(out.code.contains("x-telarel-chain"), "{}", out.code);
        assert_eq!(out.map.get_source(0), Some("index.ts"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_transform_none_returning_plugin_uses_original() {
        // A transform plugin returning Ok(None) must leave the parsed
        // original in place for codegen: the code is unchanged verbatim.
        let opts: CompileOptions = CompileOptions {
            cwd: "/repo".to_string(),
            file: "index.ts".to_string(),
            code: "console.log(1);".to_string(),
        };

        let plugins: Vec<SharedPluginable> =
            vec![Arc::new(NoneReturningTransformPlugin)];

        let out = compile(opts, plugins).await.unwrap();

        assert!(out.code.contains("console.log(1);"), "{}", out.code);
        assert_eq!(out.map.get_source(0), Some("index.ts"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_transform_mixed_chain_none_then_replace() {
        // None, replace, None: only the replacing plugin's output must reach
        // codegen, and the surrounding Nones must not reset to the original.
        let opts: CompileOptions = CompileOptions {
            cwd: "/repo".to_string(),
            file: "index.ts".to_string(),
            code: "console.log(1);".to_string(),
        };

        let plugins: Vec<SharedPluginable> = vec![
            Arc::new(NoneReturningTransformPlugin),
            Arc::new(RenameCalleePlugin),
            Arc::new(NoneReturningTransformPlugin),
        ];

        let out = compile(opts, plugins).await.unwrap();

        assert!(out.code.contains("consolex"), "{}", out.code);
        assert_eq!(out.map.get_source(0), Some("index.ts"));
    }
}
