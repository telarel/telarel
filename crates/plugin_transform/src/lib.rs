//! # Telarel Plugin Transform
//!
//! A transform plugin for the compiler.
//!
//! This crate implements [`telarel_plugin::Plugin`] over the oxc transformer:
//! it resolves telarel-owned options into oxc [`TransformOptions`], then runs
//! the TypeScript, JSX, and syntax-lowering pipeline on the program in place.

mod helpers;
mod options;
mod plugin;

pub mod oxc {
    pub use oxc::transformer::*;
}

pub use crate::options::{
    DefineOptions, HelperLoaderMode, HelperLoaderOptions, InjectEntry,
    InjectOptions, InjectSpecifier, JsxOptions, JsxRuntime, TransformOptions,
    TransformTarget, TypeScriptOptions,
};
pub use crate::plugin::{NAME, TransformPlugin};
