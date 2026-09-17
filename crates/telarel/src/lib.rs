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

pub mod sourcemap {
    pub use oxc_sourcemap::*;
}

pub use telarel_common::{
    CodegenOptions, CodegenResult, CompileContext, CompileError,
    CompileOptions, HookUsage, ParseOptions, ParseResult,
    PartialCompileOptions,
};

pub use telarel_plugin::{
    Plugin, Pluginable, PostArgs, PreArgs, SharedPluginable, TransformArgs,
    TransformOutput, TransformReturn,
};

pub use telarel_core::{CompileOutput, compile};
