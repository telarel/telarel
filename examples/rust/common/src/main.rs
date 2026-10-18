use telarel::{CodeResult, CompileOptions, compile};

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let code: String =
        "const value: number = 1;\nasync function run(): Promise<void> { await work(); }\n".to_string();

    let result: CodeResult = compile(code, CompileOptions::default(), vec![])
        .await
        .expect("compile succeeds");

    println!("\n{}\n", result.code);
}
