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
    CompileOptions, HookUsage, Language, ParseOptions, ParseResult,
    PluginState, ResolvedOptions, SourceMap, SourceType, compose_maps,
};

pub use telarel_plugin::{
    CommonPluginContext, CompileEndArgs, CompileStartArgs, FinalizeArgs,
    FinalizeOutput, FinalizeReturn, ModuleInfo, NotifyReturn, OptionsArgs,
    OptionsReturn, Plugin, PluginContext, Pluginable, PrepareArgs,
    PrepareOutput, PrepareReturn, SharedPluginable, TransformArgs,
    TransformOutput, TransformReturn,
};

pub use telarel_core::{CompileOutput, compile};
