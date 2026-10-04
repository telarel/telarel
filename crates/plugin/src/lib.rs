//! # Telarel Plugin
//!
//! A plugin interface for the compiler.
//!
//! This crate provides the plugin traits, hook argument types, and the
//! plugin driver that runs plugin hooks across the pipeline.

mod _types;
mod options;
mod plugin;
mod plugin_driver;

pub use telarel_common::HookUsage;

pub use crate::_types::context::{
    CommonPluginContext, ModuleInfo, PluginContext,
};
pub use crate::_types::hooks::compile_end::CompileEndArgs;
pub use crate::_types::hooks::compile_start::CompileStartArgs;
pub use crate::_types::hooks::finalize::FinalizeReturn;
pub use crate::_types::hooks::finalize::{FinalizeArgs, FinalizeOutput};
pub use crate::_types::hooks::notify::NotifyReturn;
pub use crate::_types::hooks::options::{OptionsArgs, OptionsReturn};
pub use crate::_types::hooks::prepare::PrepareReturn;
pub use crate::_types::hooks::prepare::{PrepareArgs, PrepareOutput};
pub use crate::_types::hooks::transform::TransformReturn;
pub use crate::_types::hooks::transform::{TransformArgs, TransformOutput};
pub use crate::_types::order::{PluginHookMeta, PluginOrder};
pub use crate::_types::sourcemap::SourceMap;
pub use crate::options::fixpoint::options_fixpoint;
pub use crate::plugin::Plugin;
pub use crate::plugin::pluginable::{Pluginable, SharedPluginable};

#[doc(hidden)]
pub mod __internal {
    pub use crate::plugin::pluginable::{HookFuture, LocalHookFuture};
    pub use crate::plugin_driver::PluginDriver;
    pub use crate::plugin_driver::hooks::finalize::FinalizeFold;
    pub use crate::plugin_driver::hooks::prepare::PrepareFold;
}
