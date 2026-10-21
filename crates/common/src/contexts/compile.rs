use std::path::Path;

/// Context carried through a compile run; holds the ORIGINAL inputs.
#[derive(Debug)]
pub struct CompileContext<'a> {
    /// Working directory used to resolve relative paths.
    pub cwd: &'a Path,
    /// The file being compiled.
    pub file: &'a str,
    /// Original source code text.
    pub code: &'a str,
}

impl<'a> CompileContext<'a> {
    /// Create a context.
    pub fn new(
        cwd: &'a Path,
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
            CompileContext::new(Path::new("/repo"), "a.js", "let a;");

        assert_eq!(ctx.cwd, Path::new("/repo"));
        assert_eq!(ctx.file, "a.js");
        assert_eq!(ctx.code, "let a;");
    }
}
