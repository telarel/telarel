use crate::_types::sourcemap::SourceMap;

/// Arguments for the `compile_end` hook.
#[derive(Debug, Clone)]
pub struct CompileEndArgs {
    /// Compiled code; the last good state on error.
    pub code: String,
    /// Source map; the last good state on error.
    pub map: Option<SourceMap>,
    /// The error, when the compile failed.
    pub err: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compile_end_args_on_success() {
        let args: CompileEndArgs = CompileEndArgs {
            code: String::from("let a;"),
            map: None,
            err: None,
        };

        assert_eq!(args.code, "let a;");
        assert!(args.map.is_none());
        assert!(args.err.is_none());
    }

    #[test]
    fn test_compile_end_args_on_error() {
        let args: CompileEndArgs = CompileEndArgs {
            code: String::from(""),
            map: None,
            err: Some(String::from("boom")),
        };

        assert_eq!(args.err.as_deref(), Some("boom"));
    }
}
