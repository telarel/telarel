//! # Telarel Plugin Transform
//!
//! A transform plugin for the compiler.
//!
//! Implements [`telarel_plugin::Plugin`] over the oxc transformer: resolves
//! telarel-owned options into oxc [`TransformOptions`], clones the read-only
//! [`telarel_common::Ast`] into a fresh owned AST, and runs the TypeScript,
//! JSX, and syntax-lowering pipeline over that clone through
//! [`telarel_common::Ast::with_mut`], returning it when it changed.

mod helpers;
mod options;
mod plugin;

pub mod oxc {
    pub use oxc::transformer::*;
}

pub use crate::options::TransformOptions;
pub use crate::options::define::DefineOptions;
pub use crate::options::helper_loader::{
    HelperLoaderMode, HelperLoaderOptions,
};
pub use crate::options::inject::{InjectEntry, InjectOptions, InjectSpecifier};
pub use crate::options::jsx::{JsxOptions, JsxRuntime};
pub use crate::options::target::TransformTarget;
pub use crate::options::typescript::TypeScriptOptions;
pub use crate::plugin::{NAME, TransformPlugin};
