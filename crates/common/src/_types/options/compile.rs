use crate::_types::options::language::Language;
use crate::_types::options::source_type::SourceType;

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
    /// The grammar of the code;
    /// `None` infers the grammar from the file extension.
    pub language: Option<Language>,
    /// The module system of the code;
    /// `None` keeps the module kind resolved from the grammar.
    pub source_type: Option<SourceType>,
}

impl Default for CompileOptions {
    fn default() -> Self {
        Self {
            cwd: None,
            file: String::from("index.js"),
            code: String::from(""),
            language: None,
            source_type: None,
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
            language: None,
            source_type: None,
        };

        assert_eq!(options.cwd.as_deref(), Some("/repo"));
        assert_eq!(options.file, "src/app.js");
        assert_eq!(options.code, "let a = 1;");
        assert_eq!(options.language, None);
        assert_eq!(options.source_type, None);
    }
}
