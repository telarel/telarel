use telarel::{CodeResult, CompileOptions, Plugin, compile};
use telarel_plugin_transform::{
    TransformOptions, TransformPlugin, TransformTarget,
};

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let transform_plugin: TransformPlugin =
        TransformPlugin::with_options(TransformOptions {
            targets: vec![TransformTarget::Es2015],
            ..TransformOptions::default()
        });

    let code: String =
        "const value: number = 1;\nasync function run(): Promise<void> { await work(); }\n".to_string();

    let result: CodeResult = compile(
        code,
        CompileOptions::default(),
        vec![Plugin::new_shared(transform_plugin)],
    )
    .await
    .expect("compile succeeds");

    println!("\n{}\n", result.code);
}
