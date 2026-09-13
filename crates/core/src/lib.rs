//! # Telarel Core
//!
//! A pipeline orchestrator for the compiler.
//!
//! This crate drives the per-file compile pipeline: it runs plugin hooks,
//! parses the source, applies transforms, and produces code with a map.

use oxc::allocator::Allocator;
use oxc::ast::ast::Program;
use oxc_sourcemap::SourceMap;

use telarel_common::{
    CompileContext, CompileError, CompileOptions, ParseOptions, parse,
};
use telarel_plugin::__internal::PluginDriver;
use telarel_plugin::{PostArgs, PreArgs, SharedPluginable, TransformArgs};

fn strip_trailing_newline(source: String) -> String {
    let trimmed: &str = source.trim_end_matches('\n');
    String::from(trimmed)
}

/// Result of a successful compile.
#[derive(Debug, Clone)]
pub struct CompileOutput {
    /// Generated code.
    pub code: String,
    /// Source map.
    pub map: SourceMap<'static>,
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

    let allocator: Allocator = Allocator::default();

    let ctx: CompileContext<'_> =
        CompileContext::new(&resolved.cwd, &resolved.file, &resolved.code);

    driver
        .pre(&ctx, &PreArgs { file: &resolved.file, code: &resolved.code })
        .await
        .map_err(|error| {
            CompileError::from_message(&format!("pre hook: {error:#}"))
        })?;

    // Parse ONCE before the transform chain. Root the resolved source in the
    // allocator so the parsed program borrows from the same allocation.
    let source: &'_ str = allocator.alloc_str(&resolved.code);

    let parse_options: ParseOptions<'_, '_> = ParseOptions {
        context: &ctx,
        allocator: &allocator,
        file: &resolved.file,
        code: source,
    };

    let original: Program<'_> = parse(parse_options)?.program;

    // Transform stage over the native Program.
    let transform_args: TransformArgs<'_> = TransformArgs {
        allocator: &allocator,
        file: &resolved.file,
        program: &original,
    };

    let replaced: Option<Program<'_>> =
        driver.transform(&ctx, &transform_args).await.map_err(|error| {
            CompileError::from_message(&format!("transform hook: {error:#}"))
        })?;

    // Pointer stability is the DRIVER's job (it allocs replacements in
    // `args.allocator`); the core only borrows for codegen. `Program` is not
    // `Clone` — do NOT copy it here.
    let current: &Program<'_> = replaced.as_ref().unwrap_or(&original);

    // Codegen from the AST (original when no plugin replaced).
    let result: telarel_common::CodegenResult<'_> =
        telarel_common::codegen(telarel_common::CodegenOptions {
            file: &resolved.file,
            program: current,
        });

    let final_source: String = strip_trailing_newline(result.code);

    let map: SourceMap<'static> = result.map.into_owned();

    driver
        .post(&ctx, &PostArgs { file: &resolved.file, code: &final_source })
        .await
        .map_err(|error| {
            CompileError::from_message(&format!("post hook: {error:#}"))
        })?;

    Ok(CompileOutput { code: final_source, map })
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;
    use std::sync::Arc;

    use telarel_common::CompileContext;
    use telarel_common::CompileOptions;
    use telarel_plugin::{
        Plugin, SharedPluginable, TransformArgs, TransformOutput,
        TransformReturn,
    };

    use oxc::allocator::{ArenaBox, ArenaVec, CloneIn, GetAllocator};
    use oxc::ast::ast::{
        Argument, CallExpression, Directive, Expression, ExpressionStatement,
        IdentifierReference, NumericLiteral, Program, Statement,
        StaticMemberExpression,
    };
    use oxc::ast::builder::AstBuilder;
    use oxc::str::Ident;

    use super::*;

    #[derive(Debug)]
    struct NoopPlugin;

    impl Plugin for NoopPlugin {
        fn name(&self) -> Cow<'static, str> {
            "noop".into()
        }
    }

    const TARGET: &str = "console";

    const REPLACEMENT: &str = "consolex";

    #[derive(Debug)]
    struct RenameCalleePlugin;

    impl Plugin for RenameCalleePlugin {
        fn name(&self) -> Cow<'static, str> {
            "rename-callee".into()
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
        let map: SourceMap<'static> = out.map;
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
        let map: SourceMap<'static> = out.map;
        assert_eq!(map.get_source(0), Some("index.ts"));
        assert!(
            map.get_tokens().next().is_some(),
            "mappings must be non-empty"
        );
    }
}
