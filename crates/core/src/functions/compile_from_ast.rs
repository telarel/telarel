use telarel_common::{
    Ast, CodeResult, CompileError, CompileFromAstOptions, ResolvableOptions,
};
use telarel_plugin::SharedPluginable;

use crate::common::pipeline::{CompileFromAstStage, run};

/// Compile an existing `ast` through rewrite and generation.
///
/// Runs `options`, `start`, `prepare`, `transform`, `finalize`, `generate`,
/// and `end` inline. Thus `options`, `start`, and `end` fire exactly once.
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

    use telarel_common::{Ast, CodeResult, CompileFromAstOptions};
    use telarel_plugin::{Plugin, SharedPluginable};

    use crate::common::test_util::{
        ReplaceAstPlugin, SpyPlugin, parse_fixture,
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
}
