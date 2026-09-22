//! # Telarel
//!
//! An extensible JavaScript compiler.
//!
//! This crate is the public-facing API, re-exporting the compile pipeline,
//! the plugin system, and the shared types.

pub mod allocator {
    pub use oxc::allocator::*;
}

pub mod ast {
    pub use oxc::ast::*;
}

pub mod str {
    pub use oxc::str::*;
}

pub mod span {
    pub use oxc::span::*;
}

pub mod syntax {
    pub use oxc::syntax::*;
}

#[cfg(feature = "ast_visit")]
pub mod ast_visit {
    pub use oxc::ast_visit::*;
}

#[cfg(feature = "semantic")]
pub mod semantic {
    pub use oxc::semantic::*;
}

#[cfg(feature = "traverse")]
pub mod traverse {
    pub use oxc_traverse::*;
}

pub mod sourcemap {
    pub use oxc_sourcemap::*;
}

pub use telarel_common::{
    CodegenOptions, CodegenResult, CompileContext, CompileError,
    CompileOptions, HookUsage, ParseOptions, ParseResult,
};

pub use telarel_plugin::{
    Plugin, Pluginable, PostArgs, PreArgs, SharedPluginable, TransformArgs,
    TransformReturn,
};

pub use telarel_core::{CompileOutput, compile};
