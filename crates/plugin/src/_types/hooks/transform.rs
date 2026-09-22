use oxc::allocator::Allocator;
use oxc::ast::ast::Program;

/// Arguments for the `transform` hook.
///
/// `'a` is the lifetime of the hook call; `'ast` is the lifetime of the
/// program inside the compile allocator. They are independent: the driver
/// reborrows the program for each plugin in the chain.
pub struct TransformArgs<'a, 'ast> {
    /// The allocator the program is rooted in; interned strings and new
    /// nodes must be allocated here.
    pub allocator: &'ast Allocator,
    /// The file being compiled.
    pub file: &'a str,
    /// The current program; plugins mutate it in place. A plugin may swap
    /// the entire root by assigning a freshly parsed `Program` over it.
    pub program: &'a mut Program<'ast>,
}

/// Return type of the `transform` hook.
pub type TransformReturn = anyhow::Result<()>;
