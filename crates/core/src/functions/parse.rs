use telarel_common::{
    AstResult, CompileError, ParseOptions, ResolvableOptions,
};
use telarel_plugin::SharedPluginable;

use crate::common::pipeline::{ParseStage, run};

/// Parse `code` into an AST.
///
/// Runs `options`, `start`, `parse`, and `end` — the builtin parser resolves
/// any requested input source map and is the terminal fallback after every
/// `parse` hook defers.
pub async fn parse(
    code: String,
    options: ParseOptions,
    plugins: Vec<SharedPluginable>,
) -> Result<AstResult, CompileError> {
    run(ParseStage { code }, ResolvableOptions::from(options), plugins).await
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::Mutex;

    use telarel_common::{AstResult, CompileError, ParseOptions, SourceMap};
    use telarel_plugin::{Plugin, SharedPluginable};

    use crate::common::test_util::{
        EndObserverPlugin, FailPlugin, FailStage, ParseOutcome,
        ParseOutcomePlugin, SpyPlugin, assert_same_map, input_value,
        sample_map,
    };

    use super::parse;

    #[tokio::test(flavor = "current_thread")]
    async fn test_parse_runs_only_its_hooks_once() {
        let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(SpyPlugin::new("spy", &log))];

        let result: AstResult = parse(
            String::from("const a = 1;"),
            ParseOptions::default(),
            plugins,
        )
        .await
        .expect("parse succeeds");

        assert_eq!(result.ast.source().as_ref(), "const a = 1;");
        assert!(result.ast.map().is_none());

        assert_eq!(
            *log.lock().unwrap(),
            vec![
                String::from("spy:options"),
                String::from("spy:start"),
                String::from("spy:parse"),
                String::from("spy:end"),
            ],
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_parse_attaches_value_input_map() {
        let map: SourceMap = sample_map("upstream.ts", 5);

        let options: ParseOptions = ParseOptions {
            source_map: Some(input_value(map.clone())),
            ..ParseOptions::default()
        };

        let result: AstResult =
            parse(String::from("const a = 1;"), options, vec![])
                .await
                .expect("parse succeeds");

        assert_same_map(result.ast.map().expect("map attached"), &map);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_end_receives_none_on_success() {
        let ended: Arc<Mutex<Vec<Option<String>>>> =
            Arc::new(Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(EndObserverPlugin::new(&ended))];

        parse(String::from("const a = 1;"), ParseOptions::default(), plugins)
            .await
            .expect("parse succeeds");

        assert_eq!(*ended.lock().unwrap(), vec![None]);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_end_receives_the_start_error() {
        let ended: Arc<Mutex<Vec<Option<String>>>> =
            Arc::new(Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(EndObserverPlugin::new(&ended)),
            Plugin::new_shared(FailPlugin::new("fail-start", FailStage::Start)),
        ];

        let error: CompileError = parse(
            String::from("const a = 1;"),
            ParseOptions::default(),
            plugins,
        )
        .await
        .expect_err("start failure fails the invocation");

        assert_eq!(error.message(), "fail-start");

        let observed: Vec<Option<String>> = ended.lock().unwrap().clone();

        assert_eq!(observed.len(), 1);

        assert_eq!(observed[0].as_deref(), Some("fail-start"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_end_receives_the_parse_error() {
        let ended: Arc<Mutex<Vec<Option<String>>>> =
            Arc::new(Mutex::new(Vec::new()));

        let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(EndObserverPlugin::new(&ended)),
            Plugin::new_shared(ParseOutcomePlugin::new(
                "fail-parse",
                ParseOutcome::Fail,
                &log,
            )),
        ];

        let error: CompileError = parse(
            String::from("const a = 1;"),
            ParseOptions::default(),
            plugins,
        )
        .await
        .expect_err("parse failure fails the invocation");

        assert_eq!(error.message(), "boom");

        let observed: Vec<Option<String>> = ended.lock().unwrap().clone();

        assert_eq!(observed.len(), 1);

        assert_eq!(observed[0].as_deref(), Some("boom"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_parse_defer_then_win() {
        let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(ParseOutcomePlugin::new(
                "a",
                ParseOutcome::Defer,
                &log,
            )),
            Plugin::new_shared(ParseOutcomePlugin::new(
                "b",
                ParseOutcome::Win("let b;"),
                &log,
            )),
        ];

        let result: AstResult = parse(
            String::from("let builtin;"),
            ParseOptions::default(),
            plugins,
        )
        .await
        .expect("parse succeeds");

        assert_eq!(result.ast.source().as_ref(), "let b;");

        assert_eq!(
            *log.lock().unwrap(),
            vec![String::from("a"), String::from("b")],
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_parse_first_win_skips_later_hooks() {
        let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(ParseOutcomePlugin::new(
                "a",
                ParseOutcome::Win("let a;"),
                &log,
            )),
            Plugin::new_shared(ParseOutcomePlugin::new(
                "b",
                ParseOutcome::Win("let b;"),
                &log,
            )),
        ];

        let result: AstResult = parse(
            String::from("let builtin;"),
            ParseOptions::default(),
            plugins,
        )
        .await
        .expect("parse succeeds");

        assert_eq!(result.ast.source().as_ref(), "let a;");

        assert_eq!(*log.lock().unwrap(), vec![String::from("a")]);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_parse_error_aborts_before_later_hooks() {
        let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(ParseOutcomePlugin::new(
                "a",
                ParseOutcome::Defer,
                &log,
            )),
            Plugin::new_shared(ParseOutcomePlugin::new(
                "fail-parse",
                ParseOutcome::Fail,
                &log,
            )),
            Plugin::new_shared(ParseOutcomePlugin::new(
                "never",
                ParseOutcome::Win("let never;"),
                &log,
            )),
        ];

        let error: CompileError = parse(
            String::from("let builtin;"),
            ParseOptions::default(),
            plugins,
        )
        .await
        .expect_err("a parse hook error aborts");

        assert_eq!(error.message(), "boom");

        assert_eq!(
            *log.lock().unwrap(),
            vec![String::from("a"), String::from("fail-parse")],
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_parse_all_defer_falls_back_to_builtin() {
        let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(ParseOutcomePlugin::new(
                "a",
                ParseOutcome::Defer,
                &log,
            )),
            Plugin::new_shared(ParseOutcomePlugin::new(
                "b",
                ParseOutcome::Defer,
                &log,
            )),
        ];

        let result: AstResult = parse(
            String::from("let builtin;"),
            ParseOptions::default(),
            plugins,
        )
        .await
        .expect("parse succeeds");

        assert_eq!(result.ast.source().as_ref(), "let builtin;");
    }
}
