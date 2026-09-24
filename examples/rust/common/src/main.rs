use telarel::{CompileOptions, CompileOutput, compile};

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let options: CompileOptions = CompileOptions {
        cwd: std::env::current_dir().unwrap().to_string_lossy().into_owned(),
        file: "index.ts".to_string(),
        code: "const value: number = 1;\nasync function run(): Promise<void> { await work(); }\n".to_string(),
    };

    let output: CompileOutput =
        compile(options, vec![]).await.expect("compile succeeds");

    println!("\n{}\n", output.code);
}
