//! # Telarel Plugin
//!
//! A plugin interface for the compiler.
//!
//! This crate provides the plugin traits, hook argument types, and the
//! plugin driver that runs plugin hooks across the pipeline.

mod _types;
mod plugin;
mod plugin_driver;

pub use _types::hooks::post::PostArgs;
pub use _types::hooks::pre::PreArgs;
pub use _types::hooks::transform::{TransformArgs, TransformReturn};
pub use plugin::Plugin;
pub use plugin::pluginable::{Pluginable, SharedPluginable};
pub use telarel_common::{HookUsage, PartialCompileOptions};

#[doc(hidden)]
pub mod __internal {
    pub use crate::plugin::pluginable::{HookFuture, LocalHookFuture};
    pub use crate::plugin_driver::PluginDriver;
}
