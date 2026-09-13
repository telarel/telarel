//! # Telarel
//!
//! An extensible JavaScript compiler.
//!
//! This crate is the public-facing API, re-exporting the compile pipeline,
//! the plugin system, and the shared types.

pub use telarel_common::{
    CodegenOptions, CodegenResult, CompileContext, CompileError,
    CompileOptions, ParseOptions, ParseResult,
};
pub use telarel_core::{CompileOutput, compile};
pub use telarel_plugin::{
    OptionsArgs, OptionsOutput, Plugin, Pluginable, PostArgs, PreArgs,
    SharedPluginable, TransformArgs, TransformOutput, TransformReturn,
};
