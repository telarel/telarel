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

impl Default for CompileOptions {
    fn default() -> Self {
        Self {
            cwd: String::from("/"),
            file: String::from("index.js"),
            code: String::from(""),
        }
    }
}

/// Partial update of the compile options; `None` fields keep current values.
#[derive(Debug, Clone, Default)]
pub struct PartialCompileOptions {
    /// Updated working directory, if provided.
    pub cwd: Option<String>,
    /// Updated file, if provided.
    pub file: Option<String>,
    /// Updated code, if provided.
    pub code: Option<String>,
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
