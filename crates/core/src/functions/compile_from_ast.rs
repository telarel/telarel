use telarel_common::{
    Ast, CodeResult, CompileError, CompileFromAstOptions, ResolvableOptions,
};
use telarel_plugin::SharedPluginable;

use crate::common::pipeline::{CompileFromAstStage, run};

/// Compile an existing `ast` through rewrite and generation.
///
/// Runs `options`, `start`, `prepare`, `transform`, `finalize`, `generate`,
/// and `end` inline. Thus `options`, `start`, and `end` fire exactly once. An
/// explicit input-map request applies at the entry boundary and replaces the
/// AST's own map when it resolves; an `inline` or `auto` miss keeps that map,
/// and an `external` miss rejects. A map returned by a `transform` hook is
/// downstream state and is not replaced by the request. Output delivery is
/// applied to the winning result.
pub async fn compile_from_ast(
    ast: Ast,
    options: CompileFromAstOptions,
    plugins: Vec<SharedPluginable>,
) -> Result<CodeResult, CompileError> {
    run(CompileFromAstStage { ast }, ResolvableOptions::from(options), plugins)
        .await
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::Mutex;

    use telarel_common::{
        Ast, CodeResult, CompileFromAstOptions, ResolvedOptions, SourceMap,
        SourceType,
    };
    use telarel_plugin::{Plugin, SharedPluginable};

    use crate::common::test_util::{
        ReplaceAstPlugin, SpyPlugin, StartObserverPlugin, first_token,
        input_value, parse_as, parse_fixture, parse_with_map, sample_map,
    };

    use super::compile_from_ast;

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_from_ast_runs_full_chain_once() {
        let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let ast: Ast = parse_fixture("const a = 1;").unwrap();

        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(SpyPlugin::new("spy", &log))];

        let result: CodeResult =
            compile_from_ast(ast, CompileFromAstOptions::default(), plugins)
                .await
                .expect("compile_from_ast succeeds");

        assert_eq!(result.code, "const a = 1;");

        assert_eq!(
            *log.lock().unwrap(),
            vec![
                String::from("spy:options"),
                String::from("spy:start"),
                String::from("spy:prepare"),
                String::from("spy:transform"),
                String::from("spy:finalize"),
                String::from("spy:generate"),
                String::from("spy:end"),
            ],
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_from_ast_transform_reaches_generate() {
        let ast: Ast = parse_fixture("const a = 1;").unwrap();

        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(ReplaceAstPlugin {
                code: "\"replaced\";",
            })];

        let result: CodeResult =
            compile_from_ast(ast, CompileFromAstOptions::default(), plugins)
                .await
                .expect("compile_from_ast succeeds");

        assert_eq!(result.code, "\"replaced\";");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_compile_from_ast_request_overrides_the_asts_own_map() {
        let incoming: SourceMap = sample_map("incoming.ts", 1);
        let requested: SourceMap = sample_map("requested.ts", 2);

        let ast: Ast = parse_with_map("const a = 1;", incoming);

        let options: CompileFromAstOptions = CompileFromAstOptions {
            source_map: Some(input_value(requested)),
            ..CompileFromAstOptions::default()
        };

        let result: CodeResult = compile_from_ast(ast, options, vec![])
            .await
            .expect("compile_from_ast succeeds");

        let generated: SourceMap = result.map.expect("output map present");

        assert_eq!(generated.get_source(0), Some("requested.ts"));
        assert_eq!(first_token(&generated), (0, 0, 2, 0));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_commonjs_kind_survives_an_mjs_file() {
        let seen: Arc<Mutex<Vec<ResolvedOptions>>> =
            Arc::new(Mutex::new(Vec::new()));

        let ast: Ast = parse_as("module.exports = 1;", SourceType::CommonJS);

        let options: CompileFromAstOptions = CompileFromAstOptions {
            file: Some(String::from("index.mjs")),
            ..CompileFromAstOptions::default()
        };

        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(StartObserverPlugin::new(&seen))];

        let result: CodeResult = compile_from_ast(ast, options, plugins)
            .await
            .expect("compile_from_ast succeeds");

        assert_eq!(result.code, "module.exports = 1;");
        assert_eq!(seen.lock().unwrap()[0].source_type, SourceType::CommonJS,);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_explicit_source_type_reaches_the_context() {
        let seen: Arc<Mutex<Vec<ResolvedOptions>>> =
            Arc::new(Mutex::new(Vec::new()));

        let ast: Ast = parse_as("const a = 1;", SourceType::Module);

        let options: CompileFromAstOptions = CompileFromAstOptions {
            file: Some(String::from("index.cjs")),
            source_type: Some(SourceType::Script),
            ..CompileFromAstOptions::default()
        };

        let plugins: Vec<SharedPluginable> =
            vec![Plugin::new_shared(StartObserverPlugin::new(&seen))];

        compile_from_ast(ast, options, plugins)
            .await
            .expect("compile_from_ast succeeds");

        assert_eq!(seen.lock().unwrap()[0].source_type, SourceType::Script);
    }
}
