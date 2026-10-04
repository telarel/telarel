use anyhow::Context;

use crate::_types::context::PluginContext;
use crate::_types::hooks::prepare::{PrepareArgs, PrepareOutput};
use crate::plugin::pluginable::SharedPluginable;

/// Result of the `prepare` fold: the final carried output (the code the next
/// stage parses) and every returned map collected in fold order.
#[derive(Debug)]
pub struct PrepareFold {
    /// The last `Some` output, replaced by each returning hook. Its `code`
    /// equals the final carried code, so callers can read the parse input
    /// from here.
    pub output: Option<PrepareOutput>,
    /// Every map returned since the last omitted one, in fold order
    /// (serving order); each entry is the output of a hook whose map is
    /// incremental relative to the code that hook received.
    ///
    /// A hook returning `Some(code)` without a map changes the code but
    /// provides no link in the chain: its map's src space (the code the
    /// NEXT hook would have received) never materializes as a composable
    /// dst space, and maps collected before the gap cannot resolve across
    /// it — so the collection resets there and the surviving segment
    /// references the actual parse input.
    pub maps: Vec<PrepareOutput>,
}

/// Run the `prepare` hook on every plugin in registration order; the first error
/// aborts.
///
/// This is a CARRIED fold: the accumulator (initially `initial.code`) is fed
/// to each subsequent hook, so every plugin sees the previous plugin's
/// rewrite. `Some(output)` replaces the carried code (and the returned
/// [`PrepareFold::output`]); `None` keeps it.
///
/// Every returned map is collected into [`PrepareFold::maps`] so the caller can
/// compose the WHOLE chain (not just the last one); a returning hook without
/// a map resets the collection (see [`PrepareFold::maps`]). Because the code
/// carries, each collected map is incremental relative to the code its hook
/// LITERALLY received.
pub async fn prepare(
    plugins: &[SharedPluginable],
    ctx: &PluginContext<'_>,
    initial: &PrepareArgs<'_>,
) -> anyhow::Result<PrepareFold> {
    let mut carried: String = initial.code.to_string();

    let mut output: Option<PrepareOutput> = None;

    let mut maps: Vec<PrepareOutput> = Vec::new();

    for plugin in plugins {
        let hook_args: PrepareArgs<'_> = PrepareArgs { code: &carried };

        let next: Option<PrepareOutput> = plugin
            .call_prepare(ctx, &hook_args)
            .await
            .with_context(|| format!("`{}` prepare", plugin.call_name()))?;

        match next {
            | Some(out @ PrepareOutput { map: Some(_), .. }) => {
                carried = out.code.clone();

                maps.push(out.clone());

                output = Some(out);
            },
            | Some(out) => {
                carried = out.code.clone();

                maps.clear();

                output = Some(out);
            },
            | None => {},
        }
    }

    Ok(PrepareFold { output, maps })
}

#[cfg(test)]
// The house style for the hook impls is the explicit RPITIT form (never
// `async fn`), so the `manual_async_fn` lint is suppressed here.
#[allow(clippy::manual_async_fn)]
mod tests {
    use std::borrow::Cow;
    use std::sync::Arc;
    use std::sync::Mutex;

    use oxc_sourcemap::SourceMapBuilder;

    use telarel_common::{HookUsage, Language, SourceType};

    use crate::_types::context::{CommonPluginContext, PluginContext};
    use crate::_types::hooks::prepare::{
        PrepareArgs, PrepareOutput, PrepareReturn,
    };
    use crate::plugin::Plugin;
    use crate::plugin::pluginable::SharedPluginable;

    use super::*;

    // The step receives the per-call args, so a probe plugin can observe the
    // carried code the fold handed it.
    type Step = Arc<
        dyn for<'a, 'b> Fn(&'a PrepareArgs<'b>) -> PrepareReturn + Send + Sync,
    >;

    struct StepPlugin {
        name: &'static str,
        step: Step,
    }

    impl std::fmt::Debug for StepPlugin {
        fn fmt(
            &self,
            f: &mut std::fmt::Formatter<'_>,
        ) -> std::fmt::Result {
            f.write_str(self.name)
        }
    }

    fn step_plugin(
        name: &'static str,
        step: impl for<'a, 'b> Fn(&'a PrepareArgs<'b>) -> PrepareReturn
        + Send
        + Sync
        + 'static,
    ) -> SharedPluginable {
        Plugin::new_shared(StepPlugin { name, step: Arc::new(step) })
    }

    impl Plugin for StepPlugin {
        fn name(&self) -> Cow<'static, str> {
            self.name.into()
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::Prepare
        }

        fn prepare<'a>(
            &'a self,
            _ctx: &'a PluginContext<'_>,
            args: &'a PrepareArgs<'_>,
        ) -> impl Future<Output = PrepareReturn> + Send {
            async move { (self.step)(args) }
        }
    }

    fn hook_map(tokens: &[(u32, u32, u32, u32)]) -> PrepareOutput {
        let mut builder: SourceMapBuilder<'_> = SourceMapBuilder::default();

        builder.set_file("a.ts");

        let source_id: u32 = builder.add_source_and_content("a.ts", "code");

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

        PrepareOutput {
            code: String::from("code"),
            map: Some(builder.into_owned_sourcemap().into_inner()),
        }
    }

    fn no_map(code: &str) -> PrepareOutput {
        PrepareOutput { code: code.to_string(), map: None }
    }

    async fn run(plugins: Vec<SharedPluginable>) -> PrepareFold {
        let common: &'static CommonPluginContext =
            Box::leak(Box::new(CommonPluginContext::default()));

        let module: crate::ModuleInfo<'static> = crate::ModuleInfo {
            file: "a.ts",
            code: "console.log(1);",
            language: Language::TS,
            source_type: SourceType::Module,
        };

        let ctx: PluginContext<'static> =
            PluginContext::new(&common.state, "/repo", module);

        let args: PrepareArgs<'_> = PrepareArgs { code: "console.log(1);" };

        prepare(&plugins, &ctx, &args).await.unwrap()
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_prepare_collects_maps_in_fold_order() {
        let first: PrepareOutput = hook_map(&[(1, 0, 0, 0)]);

        let second: PrepareOutput = hook_map(&[(2, 0, 0, 0)]);

        let plugins: Vec<SharedPluginable> = vec![
            step_plugin("one", move |_args| Ok(Some(first.clone()))),
            step_plugin("two", move |_args| Ok(Some(second.clone()))),
        ];

        let fold: PrepareFold = run(plugins).await;

        assert_eq!(fold.maps.len(), 2, "both maps collected");

        let first_collected: oxc_sourcemap::Token =
            fold.maps[0].map.as_ref().unwrap().get_token(0).unwrap();

        assert_eq!(first_collected.get_dst_line(), 1);

        let second_collected: oxc_sourcemap::Token =
            fold.maps[1].map.as_ref().unwrap().get_token(0).unwrap();

        assert_eq!(second_collected.get_dst_line(), 2);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_prepare_omission_resets_collected_chain() {
        let first: PrepareOutput = hook_map(&[(1, 0, 0, 0)]);

        let plugins: Vec<SharedPluginable> = vec![
            step_plugin("mapped", move |_args| Ok(Some(first.clone()))),
            step_plugin("unmapped", |_args| Ok(Some(no_map("unmapped-code")))),
            step_plugin("later", |_args| Ok(None)),
        ];

        let fold: PrepareFold = run(plugins).await;

        assert!(fold.maps.is_empty(), "omission clears the chain");

        assert_eq!(
            fold.output.as_ref().map(|out: &PrepareOutput| out.code.as_str()),
            Some("unmapped-code"),
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_prepare_none_keeps_output_and_chain() {
        let first: PrepareOutput = hook_map(&[(1, 0, 0, 0)]);

        let plugins: Vec<SharedPluginable> = vec![
            step_plugin("noop", |_args| Ok(None)),
            step_plugin("mapped", move |_args| Ok(Some(first.clone()))),
        ];

        let fold: PrepareFold = run(plugins).await;

        assert_eq!(fold.maps.len(), 1);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_prepare_carries_output_between_hooks() {
        // The fold is CARRIED: the second hook must receive the first
        // hook's returned code, not the original source. The probe records
        // what it saw; on the fixed-input behavior it would record the
        // original.
        let seen: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let probe_seen: Arc<Mutex<Vec<String>>> = Arc::clone(&seen);

        let plugins: Vec<SharedPluginable> = vec![
            step_plugin("first", |_args| Ok(Some(no_map("let a = 1;")))),
            step_plugin("probe", move |args: &PrepareArgs<'_>| {
                probe_seen.lock().unwrap().push(args.code.to_string());

                Ok(None)
            }),
        ];

        let fold: PrepareFold = run(plugins).await;

        assert_eq!(
            seen.lock().unwrap().clone(),
            vec![String::from("let a = 1;")],
            "the second hook must see the first hook's rewrite",
        );

        assert_eq!(
            fold.output.as_ref().map(|out: &PrepareOutput| out.code.as_str()),
            Some("let a = 1;"),
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_prepare_error_names_plugin() {
        let plugins: Vec<SharedPluginable> =
            vec![step_plugin("fail-prepare", |_args| {
                Err(anyhow::anyhow!("boom"))
            })];

        let common: &'static CommonPluginContext =
            Box::leak(Box::new(CommonPluginContext::default()));

        let module: crate::ModuleInfo<'static> = crate::ModuleInfo {
            file: "a.ts",
            code: "console.log(1);",
            language: Language::TS,
            source_type: SourceType::Module,
        };

        let ctx: PluginContext<'static> =
            PluginContext::new(&common.state, "/repo", module);

        let args: PrepareArgs<'_> = PrepareArgs { code: "console.log(1);" };

        let err: anyhow::Error =
            prepare(&plugins, &ctx, &args).await.unwrap_err();

        assert!(
            format!("{err:#}").contains("`fail-prepare` prepare"),
            "{err:#}"
        );
    }
}
