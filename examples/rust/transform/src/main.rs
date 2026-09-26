use telarel::{
    CompileOptions, CompileOutput, Plugin, SharedPluginable, compile,
};
use telarel_plugin_transform::{
    TransformOptions, TransformPlugin, TransformTarget,
};

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let options: CompileOptions = CompileOptions {
        file: "index.ts".to_string(),
        code: "const value: number = 1;\nasync function run(): Promise<void> { await work(); }\n".to_string(),
        ..CompileOptions::default()
    };

    let plugin: TransformPlugin =
        TransformPlugin::with_options(TransformOptions {
            targets: vec![TransformTarget::Es2015],
            ..TransformOptions::default()
        });

    let plugins: Vec<SharedPluginable> = vec![Plugin::new_shared(plugin)];

    let output: CompileOutput =
        compile(options, plugins).await.expect("compile succeeds");

    println!("\n{}\n", output.code);
}
