//! # Telarel
//!
//! An extensible JavaScript compiler.
//!
//! This crate is the public-facing API, re-exporting the compile pipeline,
//! the plugin system, and the shared types.

pub use telarel_common::{
    Ast, CodegenOptions, CodegenResult, CompileContext, CompileError,
    CompileOptions, HookUsage, Language, ParseOptions, ParseOwnedOptions,
    ParseResult, ResolvedOptions, SourceMap, SourceType, compose_maps,
    parse_owned,
};

pub use telarel_plugin::{
    CommonPluginContext, CompileEndArgs, CompileStartArgs, FinalizeArgs,
    FinalizeOutput, FinalizeReturn, ModuleInfo, NotifyReturn, OptionsArgs,
    OptionsReturn, Plugin, PluginContext, PluginHookMeta, PluginOrder,
    Pluginable, PrepareArgs, PrepareOutput, PrepareReturn, SharedPluginable,
    TransformArgs, TransformOutput, TransformReturn,
};

pub use telarel_core::{CompileOutput, compile};

pub mod allocator {
    pub use oxc::allocator::*;
}

pub mod span {
    pub use oxc::span::*;
}

pub mod str {
    pub use oxc::str::*;
}

pub mod syntax {
    pub use oxc::syntax::*;
}

pub mod ast {
    pub use oxc::ast::*;
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
