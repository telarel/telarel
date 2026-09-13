/// Arguments for the `pre` hook.
#[derive(Debug, Clone, Copy)]
pub struct PreArgs<'a> {
    /// The file to be compiled.
    pub file: &'a str,
    /// The code to be compiled.
    pub code: &'a str,
}
