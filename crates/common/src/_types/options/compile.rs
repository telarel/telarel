/// User options for a compile run.
#[derive(Debug, Clone)]
pub struct CompileOptions {
    /// Current working directory;
    /// `None` resolves to the process working directory.
    pub cwd: Option<String>,
    /// The file to be compiled.
    pub file: String,
    /// The code to be compiled.
    pub code: String,
}

impl Default for CompileOptions {
    fn default() -> Self {
        Self {
            cwd: None,
            file: String::from("index.js"),
            code: String::from(""),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constructs_compile_options() {
        let options: CompileOptions = CompileOptions {
            cwd: Some("/repo".into()),
            file: "src/app.js".into(),
            code: "let a = 1;".into(),
        };

        assert_eq!(options.cwd.as_deref(), Some("/repo"));
        assert_eq!(options.file, "src/app.js");
        assert_eq!(options.code, "let a = 1;");
    }
}
