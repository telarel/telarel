use telarel_common::{
    Ast, AstResult, CompileError, ResolvableOptions, RewriteOptions,
};
use telarel_plugin::SharedPluginable;

use crate::common::pipeline::{RewriteStage, run};

/// Rewrite `ast` through the plugin chain.
///
/// Runs `options`, `start`, `prepare`, `transform`, `finalize`, and `end`. The
/// AST's own input map wins; otherwise the resolved input source map is
/// attached before the stages run.
pub async fn rewrite(
    ast: Ast,
    options: RewriteOptions,
    plugins: Vec<SharedPluginable>,
) -> Result<AstResult, CompileError> {
    run(RewriteStage { ast }, ResolvableOptions::from(options), plugins).await
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::Mutex;

    use telarel_common::{
        Ast, AstResult, CompileError, InputSourceMap, InputSourceMapMode,
        RewriteOptions, SourceMap,
    };
    use telarel_plugin::{Plugin, SharedPluginable};

    use crate::common::test_util::{
        EndObserverPlugin, FailPlugin, FailStage, ReplaceAstPlugin, SpyPlugin,
        assert_same_map, input_map, parse_fixture, parse_with_map, sample_map,
    };

    use super::rewrite;

    #[tokio::test(flavor = "current_thread")]
    async fn test_rewrite_runs_its_hooks_once() {
        let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let ast: Ast = parse_fixture("const a = 1;").unwrap();

        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(SpyPlugin::new("spy", &log))];

        let result: AstResult =
            rewrite(ast, RewriteOptions::default(), plugins)
                .await
                .expect("rewrite succeeds");

        assert_eq!(result.ast.source().as_ref(), "const a = 1;");

        assert_eq!(
            *log.lock().unwrap(),
            vec![
                String::from("spy:options"),
                String::from("spy:start"),
                String::from("spy:prepare"),
                String::from("spy:transform"),
                String::from("spy:finalize"),
                String::from("spy:end"),
            ],
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_rewrite_transform_chain_replaces_ast() {
        let ast: Ast = parse_fixture("const a = 1;").unwrap();

        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(ReplaceAstPlugin {
                code: "\"replaced\";",
            })];

        let result: AstResult =
            rewrite(ast, RewriteOptions::default(), plugins)
                .await
                .expect("rewrite succeeds");

        assert_eq!(result.ast.source().as_ref(), "\"replaced\";");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_rewrite_attaches_input_map_only_when_absent() {
        let map: SourceMap = sample_map("upstream.ts", 7);

        let options: RewriteOptions = RewriteOptions {
            input_source_map: Some(input_map(map.clone())),
            ..RewriteOptions::default()
        };

        let ast: Ast = parse_fixture("const a = 1;").unwrap();

        let result: AstResult =
            rewrite(ast, options, vec![]).await.expect("rewrite succeeds");

        assert_same_map(result.ast.map().expect("map attached"), &map);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_rewrite_keeps_the_asts_own_map() {
        let incoming: SourceMap = sample_map("incoming.ts", 1);
        let requested: SourceMap = sample_map("requested.ts", 2);

        let ast: Ast = parse_with_map("const a = 1;", incoming.clone());

        let options: RewriteOptions = RewriteOptions {
            input_source_map: Some(input_map(requested)),
            ..RewriteOptions::default()
        };

        let result: AstResult =
            rewrite(ast, options, vec![]).await.expect("rewrite succeeds");

        assert_same_map(result.ast.map().expect("map carried"), &incoming);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_end_receives_the_prepare_error() {
        let ended: Arc<Mutex<Vec<Option<String>>>> =
            Arc::new(Mutex::new(Vec::new()));

        let ast: Ast = parse_fixture("const a = 1;").unwrap();

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(EndObserverPlugin::new(&ended)),
            Plugin::new_shared(FailPlugin::new(
                "fail-prepare",
                FailStage::Prepare,
            )),
        ];

        let error: CompileError =
            rewrite(ast, RewriteOptions::default(), plugins)
                .await
                .expect_err("prepare failure fails the invocation");

        assert_eq!(error.message(), "fail-prepare");

        let observed: Vec<Option<String>> = ended.lock().unwrap().clone();

        assert_eq!(observed[0].as_deref(), Some("fail-prepare"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_end_receives_the_transform_error() {
        let ended: Arc<Mutex<Vec<Option<String>>>> =
            Arc::new(Mutex::new(Vec::new()));

        let ast: Ast = parse_fixture("const a = 1;").unwrap();

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(EndObserverPlugin::new(&ended)),
            Plugin::new_shared(FailPlugin::new(
                "fail-transform",
                FailStage::Transform,
            )),
        ];

        let error: CompileError =
            rewrite(ast, RewriteOptions::default(), plugins)
                .await
                .expect_err("transform failure fails the invocation");

        assert_eq!(error.message(), "fail-transform");

        let observed: Vec<Option<String>> = ended.lock().unwrap().clone();

        assert_eq!(observed[0].as_deref(), Some("fail-transform"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_end_receives_the_finalize_error() {
        let ended: Arc<Mutex<Vec<Option<String>>>> =
            Arc::new(Mutex::new(Vec::new()));

        let ast: Ast = parse_fixture("const a = 1;").unwrap();

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(EndObserverPlugin::new(&ended)),
            Plugin::new_shared(FailPlugin::new(
                "fail-finalize",
                FailStage::Finalize,
            )),
        ];

        let error: CompileError =
            rewrite(ast, RewriteOptions::default(), plugins)
                .await
                .expect_err("finalize failure fails the invocation");

        assert_eq!(error.message(), "fail-finalize");

        let observed: Vec<Option<String>> = ended.lock().unwrap().clone();

        assert_eq!(observed[0].as_deref(), Some("fail-finalize"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_end_observes_an_input_map_attach_failure() {
        let ended: Arc<Mutex<Vec<Option<String>>>> =
            Arc::new(Mutex::new(Vec::new()));

        let ast: Ast = parse_fixture("const a = 1;").unwrap();

        let options: RewriteOptions = RewriteOptions {
            cwd: Some(PathBuf::from("/definitely/missing")),
            file: Some(String::from("index.ts")),
            input_source_map: Some(InputSourceMap::Mode(
                InputSourceMapMode::External,
            )),
            ..RewriteOptions::default()
        };

        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(EndObserverPlugin::new(&ended))];

        rewrite(ast, options, plugins)
            .await
            .expect_err("a missing external map fails the rewrite");

        let observed: Vec<Option<String>> = ended.lock().unwrap().clone();

        assert_eq!(observed.len(), 1);

        assert!(
            observed[0]
                .as_deref()
                .expect("end saw error")
                .contains("cannot read source map",),
            "{:?}",
            observed[0],
        );
    }
}
