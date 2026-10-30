use telarel_common::{
    AstResult, CompileError, ParseOptions, ResolvableOptions,
};
use telarel_plugin::SharedPluginable;

use crate::common::pipeline::{ParseStage, run};

/// Parse `code` into an AST.
///
/// Runs `options`, `start`, `parse`, and `end`. An explicit input-map request
/// replaces the AST's own map when it resolves. An `inline` or `auto` request
/// that finds no map keeps that map, and an `external` request that cannot
/// read its map rejects. The builtin parser is the terminal fallback after
/// every `parse` hook defers.
pub async fn parse(
    code: String,
    options: ParseOptions,
    plugins: Vec<SharedPluginable>,
) -> Result<AstResult, CompileError> {
    run(ParseStage { code }, ResolvableOptions::from(options), plugins).await
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::Mutex;

    use tempfile::TempDir;

    use telarel_common::{
        AstResult, CompileError, InputSourceMap, InputSourceMapMode,
        ParseOptions, SourceMap, append_inline_source_map,
    };
    use telarel_plugin::{Plugin, SharedPluginable};

    use crate::common::test_util::{
        EndObserverPlugin, FailPlugin, FailStage, ParseOutcome,
        ParseOutcomePlugin, ParseWithMapPlugin, SpyPlugin, assert_same_map,
        input_map, path_buf, sample_map, temp_dir,
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
            input_source_map: Some(input_map(map.clone())),
            ..ParseOptions::default()
        };

        let result: AstResult =
            parse(String::from("const a = 1;"), options, vec![])
                .await
                .expect("parse succeeds");

        assert_same_map(result.ast.map().expect("map attached"), &map);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_parse_attaches_input_map_after_winning_parse_hook() {
        let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let map: SourceMap = sample_map("upstream.ts", 5);

        let options: ParseOptions = ParseOptions {
            input_source_map: Some(input_map(map.clone())),
            ..ParseOptions::default()
        };
        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(ParseOutcomePlugin::new(
                "winning-parse",
                ParseOutcome::Win("let b;"),
                &log,
            ))];

        let result: AstResult =
            parse(String::from("let builtin;"), options, plugins)
                .await
                .expect("parse succeeds");

        assert_eq!(result.ast.source().as_ref(), "let b;");
        assert_same_map(result.ast.map().expect("map attached"), &map);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_parse_request_overrides_winning_parse_hook_ast_map() {
        let own: SourceMap = sample_map("own.ts", 1);
        let requested: SourceMap = sample_map("requested.ts", 2);

        let options: ParseOptions = ParseOptions {
            input_source_map: Some(input_map(requested.clone())),
            ..ParseOptions::default()
        };
        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(ParseWithMapPlugin::new("let b;", own))];

        let result: AstResult =
            parse(String::from("let builtin;"), options, plugins)
                .await
                .expect("parse succeeds");

        assert_same_map(result.ast.map().expect("map attached"), &requested);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_parse_best_effort_miss_keeps_winning_parse_hook_ast_map() {
        let own: SourceMap = sample_map("own.ts", 1);
        let dir: TempDir = temp_dir();

        for mode in [InputSourceMapMode::Inline, InputSourceMapMode::Auto] {
            let options: ParseOptions = ParseOptions {
                cwd: Some(path_buf(&dir)),
                file: Some(String::from("index.ts")),
                input_source_map: Some(InputSourceMap::Mode(mode)),
                ..ParseOptions::default()
            };
            let plugins: Vec<SharedPluginable> = vec![Plugin::new_shared(
                ParseWithMapPlugin::new("let b;", own.clone()),
            )];

            let result: AstResult =
                parse(String::from("let builtin;"), options, plugins)
                    .await
                    .expect("parse succeeds");

            assert_same_map(result.ast.map().expect("map carried"), &own);
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_parse_builtin_inline_request_strips_consumed_comment() {
        let map: SourceMap = sample_map("upstream.ts", 5);

        let code: String =
            append_inline_source_map(String::from("const a = 1;"), &map);

        let options: ParseOptions = ParseOptions {
            input_source_map: Some(InputSourceMap::Mode(
                InputSourceMapMode::Inline,
            )),
            ..ParseOptions::default()
        };

        let result: AstResult =
            parse(code, options, vec![]).await.expect("parse succeeds");

        assert_same_map(result.ast.map().expect("map attached"), &map);
        assert!(result.ast.source().contains("sourceMappingURL"));
        assert!(
            result.ast.program().comments.is_empty(),
            "the consumed inline comment must not survive"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_parse_reports_input_map_errors_from_winning_parse_hook() {
        let ended: Arc<Mutex<Vec<Option<String>>>> =
            Arc::new(Mutex::new(Vec::new()));
        let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let options: ParseOptions = ParseOptions {
            cwd: Some(PathBuf::from("/definitely/missing")),
            input_source_map: Some(InputSourceMap::Mode(
                InputSourceMapMode::External,
            )),
            ..ParseOptions::default()
        };
        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(EndObserverPlugin::new(&ended)),
            Plugin::new_shared(ParseOutcomePlugin::new(
                "winning-parse",
                ParseOutcome::Win("let b;"),
                &log,
            )),
        ];

        let error: CompileError =
            parse(String::from("let builtin;"), options, plugins)
                .await
                .expect_err("missing external map fails");

        assert!(
            error.to_string().contains("cannot read source map"),
            "{error}"
        );

        let observed: Vec<Option<String>> = ended.lock().unwrap().clone();
        assert_eq!(observed.len(), 1);
        assert!(
            observed[0]
                .as_deref()
                .expect("end saw the error")
                .contains("cannot read source map"),
        );
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
