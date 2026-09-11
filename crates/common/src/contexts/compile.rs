/// Context carried through a compile run; holds the ORIGINAL inputs.
#[derive(Debug)]
pub struct CompileContext<'a> {
    /// Current working directory.
    pub cwd: &'a str,
    /// The original file.
    pub file: &'a str,
    /// The original code.
    pub code: &'a str,
}

impl<'a> CompileContext<'a> {
    /// Create a context.
    pub fn new(
        cwd: &'a str,
        file: &'a str,
        code: &'a str,
    ) -> Self {
        Self { cwd, file, code }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_holds_originals() {
        let ctx: CompileContext<'_> =
            CompileContext::new("/repo", "a.js", "let a;");

        assert_eq!(ctx.cwd, "/repo");
        assert_eq!(ctx.file, "a.js");
        assert_eq!(ctx.code, "let a;");
    }
}
