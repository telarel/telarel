/// User options for a compile run.
#[derive(Debug, Clone)]
pub struct CompileOptions {
    /// Current working directory.
    pub cwd: String,
    /// The file to be compiled.
    pub file: String,
    /// The code to be compiled.
    pub code: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constructs_compile_options() {
        let options: CompileOptions = CompileOptions {
            cwd: "/repo".into(),
            file: "src/app.js".into(),
            code: "let a = 1;".into(),
        };
        assert_eq!(options.cwd, "/repo");
        assert_eq!(options.file, "src/app.js");
        assert_eq!(options.code, "let a = 1;");
    }
}
