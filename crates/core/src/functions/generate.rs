use telarel_common::{
    Ast, CodeResult, CompileError, GenerateOptions, ResolvableOptions,
};
use telarel_plugin::SharedPluginable;

use crate::common::pipeline::{GenerateStage, run};

/// Generate code from `ast`.
///
/// Runs `options`, `start`, `generate`, and `end` — the builtin generator is
/// the terminal fallback after every `generate` hook defers, and the resolved
/// output source-map mode is applied to the winning result.
pub async fn generate(
    ast: Ast,
    options: GenerateOptions,
    plugins: Vec<SharedPluginable>,
) -> Result<CodeResult, CompileError> {
    run(GenerateStage { ast }, ResolvableOptions::from(options), plugins).await
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::Mutex;

    use telarel_common::{
        Ast, CodeResult, CompileError, GenerateOptions, OutputSourceMapMode,
    };
    use telarel_plugin::{Plugin, SharedPluginable};

    use crate::common::test_util::{
        EndObserverPlugin, GenerateOutcome, GenerateOutcomePlugin, SpyPlugin,
        output_mode, parse_fixture,
    };

    use super::generate;

    #[tokio::test(flavor = "current_thread")]
    async fn test_generate_runs_only_its_hooks_once() {
        let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let ast: Ast = parse_fixture("const a = 1;").unwrap();

        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(SpyPlugin::new("spy", &log))];

        let result: CodeResult =
            generate(ast, GenerateOptions::default(), plugins)
                .await
                .expect("generate succeeds");

        assert_eq!(result.code, "const a = 1;");

        assert_eq!(
            *log.lock().unwrap(),
            vec![
                String::from("spy:options"),
                String::from("spy:start"),
                String::from("spy:generate"),
                String::from("spy:end"),
            ],
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_generate_custom_hook_first_win() {
        let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let ast: Ast = parse_fixture("const a = 1;").unwrap();

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(GenerateOutcomePlugin::new(
                "first",
                GenerateOutcome::Win("first();"),
                &log,
            )),
            Plugin::new_shared(GenerateOutcomePlugin::new(
                "second",
                GenerateOutcome::Win("second();"),
                &log,
            )),
        ];

        let result: CodeResult =
            generate(ast, GenerateOptions::default(), plugins)
                .await
                .expect("generate succeeds");

        assert_eq!(result.code, "first();");

        assert_eq!(*log.lock().unwrap(), vec![String::from("first")]);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_generate_falls_back_to_builtin() {
        let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let ast: Ast = parse_fixture("const a = 1;").unwrap();

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(GenerateOutcomePlugin::new(
                "one",
                GenerateOutcome::Defer,
                &log,
            )),
            Plugin::new_shared(GenerateOutcomePlugin::new(
                "two",
                GenerateOutcome::Defer,
                &log,
            )),
        ];

        let result: CodeResult =
            generate(ast, GenerateOptions::default(), plugins)
                .await
                .expect("generate succeeds");

        assert_eq!(result.code, "const a = 1;");

        assert_eq!(
            *log.lock().unwrap(),
            vec![String::from("one"), String::from("two")],
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_generate_custom_mapless_result_stays_mapless() {
        let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let ast: Ast = parse_fixture("const a = 1;").unwrap();

        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(GenerateOutcomePlugin::new(
                "custom",
                GenerateOutcome::Win("custom();"),
                &log,
            ))];

        let result: CodeResult =
            generate(ast, GenerateOptions::default(), plugins)
                .await
                .expect("generate succeeds");

        assert_eq!(result.code, "custom();");
        assert!(
            result.map.is_none(),
            "a winning mapless generator must not fabricate an identity map"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_generate_output_mode_external() {
        let ast: Ast = parse_fixture("const a = 1;").unwrap();

        let options: GenerateOptions = GenerateOptions {
            source_map: Some(output_mode(OutputSourceMapMode::External)),
            ..GenerateOptions::default()
        };

        let result: CodeResult =
            generate(ast, options, vec![]).await.expect("generate succeeds");

        assert!(result.map.is_some(), "external keeps the map");
        assert!(!result.code.contains("sourceMappingURL"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_generate_output_mode_inline() {
        let ast: Ast = parse_fixture("const a = 1;").unwrap();

        let options: GenerateOptions = GenerateOptions {
            source_map: Some(output_mode(OutputSourceMapMode::Inline)),
            ..GenerateOptions::default()
        };

        let result: CodeResult =
            generate(ast, options, vec![]).await.expect("generate succeeds");

        assert!(result.map.is_none(), "inline drops the map");
        assert!(result.code.starts_with("const a = 1;\n//# sourceMappingURL="));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_generate_output_mode_both() {
        let ast: Ast = parse_fixture("const a = 1;").unwrap();

        let options: GenerateOptions = GenerateOptions {
            source_map: Some(output_mode(OutputSourceMapMode::Both)),
            ..GenerateOptions::default()
        };

        let result: CodeResult =
            generate(ast, options, vec![]).await.expect("generate succeeds");

        assert!(result.map.is_some(), "both keeps the map");
        assert!(result.code.starts_with("const a = 1;\n//# sourceMappingURL="));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_end_receives_the_generate_error() {
        let ended: Arc<Mutex<Vec<Option<String>>>> =
            Arc::new(Mutex::new(Vec::new()));

        let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let ast: Ast = parse_fixture("const a = 1;").unwrap();

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(EndObserverPlugin::new(&ended)),
            Plugin::new_shared(GenerateOutcomePlugin::new(
                "fail-generate",
                GenerateOutcome::Fail,
                &log,
            )),
        ];

        let error: CompileError =
            generate(ast, GenerateOptions::default(), plugins)
                .await
                .expect_err("generate failure fails the invocation");

        assert!(error.to_string().contains("generate hook:"), "{error}");
        assert!(
            error.to_string().contains("`fail-generate` generate"),
            "{error}"
        );

        let observed: Vec<Option<String>> = ended.lock().unwrap().clone();

        assert!(
            observed[0]
                .as_deref()
                .expect("end saw error")
                .contains("generate hook:",)
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_generate_defer_then_win() {
        let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let ast: Ast = parse_fixture("const a = 1;").unwrap();

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(GenerateOutcomePlugin::new(
                "a",
                GenerateOutcome::Defer,
                &log,
            )),
            Plugin::new_shared(GenerateOutcomePlugin::new(
                "b",
                GenerateOutcome::Win("b();"),
                &log,
            )),
        ];

        let result: CodeResult =
            generate(ast, GenerateOptions::default(), plugins)
                .await
                .expect("generate succeeds");

        assert_eq!(result.code, "b();");

        assert_eq!(
            *log.lock().unwrap(),
            vec![String::from("a"), String::from("b")],
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_generate_error_aborts_before_later_hooks() {
        let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let ast: Ast = parse_fixture("const a = 1;").unwrap();

        let plugins: Vec<SharedPluginable> = vec![
            Plugin::new_shared(GenerateOutcomePlugin::new(
                "a",
                GenerateOutcome::Defer,
                &log,
            )),
            Plugin::new_shared(GenerateOutcomePlugin::new(
                "fail-generate",
                GenerateOutcome::Fail,
                &log,
            )),
            Plugin::new_shared(GenerateOutcomePlugin::new(
                "never",
                GenerateOutcome::Win("never();"),
                &log,
            )),
        ];

        let error: CompileError =
            generate(ast, GenerateOptions::default(), plugins)
                .await
                .expect_err("a generate hook error aborts");

        assert!(
            error.to_string().contains("`fail-generate` generate"),
            "{error}"
        );

        assert_eq!(
            *log.lock().unwrap(),
            vec![String::from("a"), String::from("fail-generate")],
        );
    }
}
