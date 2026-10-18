//! # Telarel Plugin Transform
//!
//! A transform plugin for the compiler.
//!
//! Wraps the oxc transformer to apply TypeScript, JSX, and syntax lowering.

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
