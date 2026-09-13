use telarel_common::CompileOptions;

/// Arguments for the `options` hook.
#[derive(Debug, Clone, Copy)]
pub struct OptionsArgs<'a> {
    /// The compile options resolved so far.
    pub options: &'a CompileOptions,
}

/// Output of the `options` hook.
#[derive(Debug, Clone)]
pub struct OptionsOutput {
    /// The replacement compile options.
    pub options: CompileOptions,
}
