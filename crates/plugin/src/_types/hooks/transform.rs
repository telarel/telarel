use oxc::allocator::Allocator;
use oxc::ast::ast::Program;

/// Arguments for the `transform` hook.
#[derive(Clone, Copy)]
pub struct TransformArgs<'a> {
    /// The allocator, replacement programs must be allocated here.
    pub allocator: &'a Allocator,
    /// The file being compiled.
    pub file: &'a str,
    /// The current program, rooted in the allocator.
    pub program: &'a Program<'a>,
}

/// Output of the `transform` hook.
#[derive(Debug)]
pub struct TransformOutput<'a> {
    /// The replacement program, rooted in the same allocator.
    pub program: Program<'a>,
}

/// Return type of the `transform` hook.
pub type TransformReturn<'a> = anyhow::Result<Option<TransformOutput<'a>>>;
