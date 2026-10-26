//! # Telarel
//!
//! An extensible JavaScript compiler.
//!
//! This crate is the public API. It re-exports the compile pipeline,
//! the plugin system, and the shared types.

pub use telarel_common::{
    Ast, AstResult, CodeResult, CompileContext, CompileError,
    CompileFromAstOptions, CompileOptions, ErrorSource, GenerateOptions,
    Generator, GeneratorOptions, HookUsage, InputSourceMap, InputSourceMapMode,
    InputSourceMapOptions, Language, OutputOptions, OutputSourceMap,
    OutputSourceMapMode, OutputSourceMapOptions, ParseOptions, Parser,
    ParserOptions, ResolvableOptions, ResolvedOptions, ResolvedOutputOptions,
    RewriteOptions, SourceMap, SourceMapOptions, SourceType,
};

pub use telarel_plugin::{
    CommonPluginContext, EndArgs, FinalizeArgs, GenerateArgs, GenerateReturn,
    ModuleInfo, NotifyReturn, OptionsArgs, OptionsReturn, ParseArgs,
    ParseReturn, Plugin, PluginContext, PluginError, PluginHookMeta,
    PluginOrder, Pluginable, PrepareArgs, SharedPluginable, StartArgs,
    TransformArgs, TransformOutput, TransformReturn,
};

pub use telarel_core::{compile, compile_from_ast, generate, parse, rewrite};

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
