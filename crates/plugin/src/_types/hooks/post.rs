/// Arguments for the `post` hook.
#[derive(Debug, Clone, Copy)]
pub struct PostArgs<'a> {
    /// The file that was compiled.
    pub file: &'a str,
    /// The code that was compiled.
    pub code: &'a str,
}
