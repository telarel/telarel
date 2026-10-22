use telarel_common::{
    CodeResult, CompileError, CompileOptions, ResolvableOptions,
};
use telarel_plugin::SharedPluginable;

use crate::common::pipeline::{CompileStage, run};

/// Compile `code` from source to generated output.
///
/// Runs the full chain — `options`, `start`, `parse`, `prepare`, `transform`,
/// `finalize`, `generate`, and `end` — inline, so `options`/`start`/`end` fire
/// exactly once. The builtin parser resolves any requested input source map.
pub async fn compile(
    code: String,
    options: CompileOptions,
    plugins: Vec<SharedPluginable>,
) -> Result<CodeResult, CompileError> {
    run(CompileStage { code }, ResolvableOptions::from(options), plugins).await
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::Mutex;

    use tempfile::TempDir;

    use telarel_common::{
        CodeResult, CompileOptions, InputSourceMap, InputSourceMapMode,
        InputSourceMapOptions, Language, OutputOptions, OutputSourceMap,
        OutputSourceMapMode, ResolvedOptions, SourceMap, SourceMapOptions,
        SourceType, append_inline_source_map,
    };
    use telarel_plugin::{Plugin, SharedPluginable};

    use crate::common::test_util::{
        SpyPlugin, StartObserverPlugin, first_token, input_value, path_buf,
        sample_map, temp_dir, write_file,
    };

    use super::compile;

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_runs_full_chain_once() {
        let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(SpyPlugin::new("spy", &log))];

        let result: CodeResult = compile(
            String::from("const a = 1;"),
            CompileOptions::default(),
            plugins,
        )
        .await
        .expect("compile succeeds");

        assert_eq!(result.code, "const a = 1;");

        assert_eq!(
            *log.lock().unwrap(),
            vec![
                String::from("spy:options"),
                String::from("spy:start"),
                String::from("spy:parse"),
                String::from("spy:prepare"),
                String::from("spy:transform"),
                String::from("spy:finalize"),
                String::from("spy:generate"),
                String::from("spy:end"),
            ],
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_value_input_map_composes_output_map() {
        let map: SourceMap = sample_map("upstream.ts", 9);

        let options: CompileOptions = CompileOptions {
            file: Some(String::from("index.ts")),
            source_map: Some(input_value(map)),
            ..CompileOptions::default()
        };

        let result: CodeResult =
            compile(String::from("const a = 1;"), options, vec![])
                .await
                .expect("compile succeeds");

        let generated: SourceMap = result.map.expect("output map present");

        assert_eq!(generated.get_source(0), Some("index.ts"));

        assert_eq!(
            first_token(&generated),
            (0, 0, 9, 0),
            "generated (0, 0) must map to the upstream original (9, 0)"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_output_external_uses_output_file() {
        let options: CompileOptions = CompileOptions {
            file: Some(String::from("src/index.ts")),
            output: Some(OutputOptions {
                file: Some(String::from("dist/index.js")),
            }),
            source_map: Some(SourceMapOptions {
                input: None,
                output: Some(OutputSourceMap::Mode(
                    OutputSourceMapMode::External,
                )),
            }),
            ..CompileOptions::default()
        };

        let result: CodeResult =
            compile(String::from("const a = 1;"), options, vec![])
                .await
                .expect("compile succeeds");

        assert_eq!(
            result.code,
            "const a = 1;\n//# sourceMappingURL=dist/index.js.map\n",
        );
        assert!(result.map.is_some());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_inline_input_map_composes_output_map() {
        let map: SourceMap = sample_map("upstream.ts", 9);

        let code: String =
            append_inline_source_map(String::from("const a = 1;"), &map);

        let options: CompileOptions = CompileOptions {
            file: Some(String::from("index.ts")),
            source_map: Some(SourceMapOptions {
                input: Some(InputSourceMap::Mode(InputSourceMapMode::Inline)),
                output: None,
            }),
            ..CompileOptions::default()
        };

        let result: CodeResult =
            compile(code, options, vec![]).await.expect("compile succeeds");

        let generated: SourceMap = result.map.expect("output map present");

        assert_eq!(first_token(&generated), (0, 0, 9, 0));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_external_input_map_from_file() {
        let dir: TempDir = temp_dir();

        let map: SourceMap = sample_map("upstream.ts", 9);

        write_file(&dir, "index.ts.map", &map.to_json_string());

        let options: CompileOptions = CompileOptions {
            cwd: Some(path_buf(&dir)),
            file: Some(String::from("index.ts")),
            source_map: Some(SourceMapOptions {
                input: Some(InputSourceMap::Mode(InputSourceMapMode::External)),
                output: None,
            }),
            ..CompileOptions::default()
        };

        let result: CodeResult =
            compile(String::from("const a = 1;"), options, vec![])
                .await
                .expect("compile succeeds");

        let generated: SourceMap = result.map.expect("output map present");

        assert_eq!(first_token(&generated), (0, 0, 9, 0));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_external_input_map_uses_custom_path() {
        let dir: TempDir = temp_dir();

        let map: SourceMap = sample_map("upstream.ts", 9);

        write_file(&dir, "custom.map", &map.to_json_string());

        let options: CompileOptions = CompileOptions {
            cwd: Some(path_buf(&dir)),
            file: Some(String::from("index.ts")),
            source_map: Some(SourceMapOptions {
                input: Some(InputSourceMap::Options(InputSourceMapOptions {
                    mode: InputSourceMapMode::External,
                    path: Some(PathBuf::from("custom.map")),
                })),
                output: None,
            }),
            ..CompileOptions::default()
        };

        let result: CodeResult =
            compile(String::from("const a = 1;"), options, vec![])
                .await
                .expect("compile succeeds");

        let generated: SourceMap = result.map.expect("output map present");

        assert_eq!(first_token(&generated), (0, 0, 9, 0));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_auto_input_map_reads_inline() {
        let map: SourceMap = sample_map("upstream.ts", 9);

        let code: String =
            append_inline_source_map(String::from("const a = 1;"), &map);

        let options: CompileOptions = CompileOptions {
            file: Some(String::from("index.ts")),
            source_map: Some(SourceMapOptions {
                input: Some(InputSourceMap::Mode(InputSourceMapMode::Auto)),
                output: None,
            }),
            ..CompileOptions::default()
        };

        let result: CodeResult =
            compile(code, options, vec![]).await.expect("compile succeeds");

        let generated: SourceMap = result.map.expect("output map present");

        assert_eq!(first_token(&generated), (0, 0, 9, 0));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_input_map_off_by_default() {
        let result: CodeResult = compile(
            String::from("const a = 1;"),
            CompileOptions::default(),
            vec![],
        )
        .await
        .expect("compile succeeds");

        let generated: SourceMap = result.map.expect("output map present");

        assert_eq!(
            first_token(&generated),
            (0, 0, 0, 0),
            "without an input map the codegen map is uncomposed"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_defaults_to_index_ts() {
        let seen: Arc<Mutex<Vec<ResolvedOptions>>> =
            Arc::new(Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(StartObserverPlugin::new(&seen))];

        compile(
            String::from("const a = 1;"),
            CompileOptions::default(),
            plugins,
        )
        .await
        .expect("compile succeeds");

        let options: Vec<ResolvedOptions> = seen.lock().unwrap().clone();

        assert_eq!(options.len(), 1);

        assert_eq!(options[0].file, "index.ts");
        assert_eq!(options[0].language, Language::TS);
        assert_eq!(options[0].source_type, SourceType::Unambiguous);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_explicit_file_language_and_source_type_win() {
        let seen: Arc<Mutex<Vec<ResolvedOptions>>> =
            Arc::new(Mutex::new(Vec::new()));

        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(StartObserverPlugin::new(&seen))];

        let options: CompileOptions = CompileOptions {
            file: Some(String::from("app.js")),
            language: Some(Language::JS),
            source_type: Some(SourceType::Script),
            ..CompileOptions::default()
        };

        compile(String::from("const a = 1;"), options, plugins)
            .await
            .expect("compile succeeds");

        let observed: Vec<ResolvedOptions> = seen.lock().unwrap().clone();

        assert_eq!(observed[0].file, "app.js");
        assert_eq!(observed[0].language, Language::JS);
        assert_eq!(observed[0].source_type, SourceType::Script);
    }

    #[test]
    fn test_compile_future_is_send_and_static() {
        fn assert_send_static<F: Send + 'static>(_: F) {}

        assert_send_static(compile(
            String::from("const a = 1;"),
            CompileOptions::default(),
            vec![],
        ));
    }
}
