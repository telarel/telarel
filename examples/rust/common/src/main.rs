use telarel::{CodeResult, CompileOptions, compile};

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let code: String =
        "const value: number = 1;\nasync function run(): Promise<void> { await work(); }\n".to_string();

    let options: CompileOptions = CompileOptions {
        file: Some("index.ts".to_string()),
        ..CompileOptions::default()
    };

    let output: CodeResult =
        compile(code, options, vec![]).await.expect("compile succeeds");

    println!("\n{}\n", output.code);
}
