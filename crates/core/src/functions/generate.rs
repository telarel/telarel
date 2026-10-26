use telarel_common::{
    Ast, CodeResult, CompileError, GenerateOptions, ResolvableOptions,
};
use telarel_plugin::SharedPluginable;

use crate::common::pipeline::{GenerateStage, run};

/// Generate code from `ast`.
///
/// Runs `options`, `start`, `generate`, and `end`. The builtin generator is
/// the terminal fallback after every `generate` hook defers. The resolved
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
        Ast, CodeResult, CompileError, GenerateOptions, OutputOptions,
        OutputSourceMap, OutputSourceMapMode, OutputSourceMapOptions,
        ResolvedOptions, SourceMapOptions, SourceType,
    };
    use telarel_plugin::{Plugin, SharedPluginable};

    use crate::common::test_util::{
        EndObserverPlugin, GenerateOutcome, GenerateOutcomePlugin, SpyPlugin,
        StartObserverPlugin, output_mode, parse_as, parse_fixture,
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
    async fn test_generate_output_mode_external_appends_default_url() {
        let ast: Ast = parse_fixture("const a = 1;").unwrap();

        let options: GenerateOptions = GenerateOptions {
            source_map: Some(output_mode(OutputSourceMapMode::External)),
            ..GenerateOptions::default()
        };

        let result: CodeResult =
            generate(ast, options, vec![]).await.expect("generate succeeds");

        assert!(result.map.is_some(), "external keeps the map");
        assert_eq!(
            result.code,
            "const a = 1;\n//# sourceMappingURL=index.ts.map\n",
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_generate_output_mode_external_uses_output_file() {
        let ast: Ast = parse_fixture("const a = 1;").unwrap();

        let options: GenerateOptions = GenerateOptions {
            output: Some(OutputOptions {
                file: Some(String::from("dist/app.js")),
            }),
            source_map: Some(output_mode(OutputSourceMapMode::External)),
            ..GenerateOptions::default()
        };

        let result: CodeResult =
            generate(ast, options, vec![]).await.expect("generate succeeds");

        assert!(result.map.is_some(), "external keeps the map");
        assert_eq!(
            result.code,
            "const a = 1;\n//# sourceMappingURL=dist/app.js.map\n",
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_generate_default_output_omits_comment() {
        let ast: Ast = parse_fixture("const a = 1;").unwrap();

        let result: CodeResult =
            generate(ast, GenerateOptions::default(), vec![])
                .await
                .expect("generate succeeds");

        assert_eq!(result.code, "const a = 1;");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_generate_output_external_appends_configured_url() {
        let ast: Ast = parse_fixture("const a = 1;").unwrap();

        let options: GenerateOptions = GenerateOptions {
            source_map: Some(SourceMapOptions {
                input: None,
                output: Some(OutputSourceMap::Options(
                    OutputSourceMapOptions {
                        mode: OutputSourceMapMode::External,
                        url: Some(String::from("dist/app.js.map")),
                    },
                )),
            }),
            ..GenerateOptions::default()
        };

        let result: CodeResult =
            generate(ast, options, vec![]).await.expect("generate succeeds");

        assert!(result.map.is_some(), "external keeps the map");

        assert_eq!(
            result.code,
            "const a = 1;\n//# sourceMappingURL=dist/app.js.map\n",
        );
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

        assert_eq!(error.message(), "boom");

        let observed: Vec<Option<String>> = ended.lock().unwrap().clone();

        assert_eq!(observed[0].as_deref(), Some("boom"));
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
    async fn test_generate_context_uses_the_ast_module_kind() {
        let seen: Arc<Mutex<Vec<ResolvedOptions>>> =
            Arc::new(Mutex::new(Vec::new()));

        let ast: Ast = parse_as("const a = 1;", SourceType::Module);

        let options: GenerateOptions = GenerateOptions {
            file: Some(String::from("index.cjs")),
            ..GenerateOptions::default()
        };

        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(StartObserverPlugin::new(&seen))];

        generate(ast, options, plugins).await.expect("generate succeeds");

        assert_eq!(seen.lock().unwrap()[0].source_type, SourceType::Module);
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

        assert_eq!(error.message(), "boom");

        assert_eq!(
            *log.lock().unwrap(),
            vec![String::from("a"), String::from("fail-generate")],
        );
    }
}
