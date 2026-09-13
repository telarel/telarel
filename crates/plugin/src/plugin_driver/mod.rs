pub mod hooks;

use oxc::ast::ast::Program;
use telarel_common::{CompileContext, CompileOptions};

use crate::_types::hooks::post::PostArgs;
use crate::_types::hooks::pre::PreArgs;
use crate::_types::hooks::transform::TransformArgs;
use crate::plugin::pluginable::SharedPluginable;

/// Drives plugins through the hooks in registration order.
pub struct PluginDriver {
    plugins: Vec<SharedPluginable>,
}

impl PluginDriver {
    /// Create a driver from plugins in registration order.
    pub fn new(plugins: Vec<SharedPluginable>) -> Self {
        Self { plugins }
    }

    /// Run the `options` hook chain; the last `Some` output wins.
    pub async fn options(
        &self,
        options: CompileOptions,
    ) -> anyhow::Result<CompileOptions> {
        hooks::options::options(&self.plugins, options).await
    }

    /// Run the `pre` hook on every plugin in registration order.
    pub async fn pre(
        &self,
        ctx: &CompileContext<'_>,
        args: &PreArgs<'_>,
    ) -> anyhow::Result<()> {
        hooks::pre::pre(&self.plugins, ctx, args).await
    }

    /// Run the `transform` hook chain; returns the final replaced
    /// [`Program`], or `None` when no plugin replaced.
    pub async fn transform<'a>(
        &'a self,
        ctx: &'a CompileContext<'a>,
        args: &'a TransformArgs<'a>,
    ) -> anyhow::Result<Option<Program<'a>>> {
        hooks::transform::transform(&self.plugins, ctx, args).await
    }

    /// Run the `post` hook on every plugin in registration order.
    pub async fn post(
        &self,
        ctx: &CompileContext<'_>,
        args: &PostArgs<'_>,
    ) -> anyhow::Result<()> {
        hooks::post::post(&self.plugins, ctx, args).await
    }
}

// The frozen driver test keeps the explicit RPITIT shape for `MarkPlugin::
// transform`; converting it to `async fn` would be a second edit to the
// frozen file, so the `manual_async_fn` lint is suppressed here instead.
#[cfg(test)]
#[allow(clippy::manual_async_fn)]
mod tests {
    use std::borrow::Cow;
    use std::future::Future;
    use std::sync::Arc;

    use telarel_common::{
        CompileContext, CompileOptions, ParseOptions, ParseResult, parse,
    };

    use oxc::allocator::Allocator;
    use oxc::ast::ast::Program;

    use crate::_types::hooks::options::{OptionsArgs, OptionsOutput};
    use crate::_types::hooks::transform::{TransformArgs, TransformOutput};
    use crate::SharedPluginable;
    use crate::plugin::Plugin;

    use super::*;

    #[derive(Debug)]
    struct OptionsPlugin;

    impl Plugin for OptionsPlugin {
        fn name(&self) -> Cow<'static, str> {
            "options".into()
        }

        async fn options(
            &self,
            args: &OptionsArgs<'_>,
        ) -> anyhow::Result<Option<OptionsOutput>> {
            let mut options: CompileOptions = args.options.clone();
            options.cwd = "/changed".to_string();
            Ok(Some(OptionsOutput { options }))
        }
    }

    #[derive(Debug)]
    struct MarkPlugin;

    impl Plugin for MarkPlugin {
        fn name(&self) -> Cow<'static, str> {
            "mark".into()
        }

        fn transform<'a>(
            &'a self,
            ctx: &'a CompileContext<'a>,
            args: &'a TransformArgs<'a>,
        ) -> impl Future<Output = crate::_types::hooks::transform::TransformReturn<'a>>
        {
            async move {
                let code: &'a str = args.allocator.alloc_str("\"mark\";");
                let options: ParseOptions<'_, '_> = ParseOptions {
                    context: ctx,
                    allocator: args.allocator,
                    file: args.file,
                    code,
                };
                let parsed: ParseResult<'_> = parse(options).unwrap();
                Ok(Some(TransformOutput { program: parsed.program }))
            }
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_options_chain_last_wins() {
        let plugins: Vec<SharedPluginable> =
            vec![Arc::new(OptionsPlugin), Arc::new(OptionsPlugin)];

        let driver: PluginDriver = PluginDriver::new(plugins);

        let options: CompileOptions = CompileOptions {
            cwd: "/repo".to_string(),
            file: "a.ts".to_string(),
            code: "console.log(1);".to_string(),
        };

        let resolved: CompileOptions = driver.options(options).await.unwrap();

        assert_eq!(resolved.cwd, "/changed");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_transform_chains_programs() {
        let allocator: Allocator = Allocator::default();

        let ctx: CompileContext<'_> =
            CompileContext::new("/repo", "a.ts", "console.log(1);");

        let options: ParseOptions<'_, '_> = ParseOptions {
            context: &ctx,
            allocator: &allocator,
            file: "a.ts",
            code: "console.log(1);",
        };

        let parsed: ParseResult<'_> = parse(options).unwrap();

        let program: Program<'_> = parsed.program;

        let args: TransformArgs<'_> = TransformArgs {
            allocator: &allocator,
            file: "a.ts",
            program: &program,
        };

        let plugins: Vec<SharedPluginable> =
            vec![Arc::new(MarkPlugin), Arc::new(MarkPlugin)];

        let driver: PluginDriver = PluginDriver::new(plugins);

        let replaced: Option<Program<'_>> =
            driver.transform(&ctx, &args).await.unwrap();

        let replaced: Program<'_> = replaced.expect("transform ran");

        // final program survives in the allocator; assert via codegen
        let out: String = oxc::codegen::Codegen::new().build(&replaced).code;

        assert!(out.contains("mark"), "{out}");
    }
}
