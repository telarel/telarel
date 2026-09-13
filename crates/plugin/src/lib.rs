mod _types;
mod plugin;
mod plugin_driver;

pub use _types::hooks::options::{OptionsArgs, OptionsOutput};
pub use _types::hooks::post::PostArgs;
pub use _types::hooks::pre::PreArgs;
pub use _types::hooks::transform::{
    TransformArgs, TransformOutput, TransformReturn,
};
pub use plugin::Plugin;
pub use plugin::pluginable::{Pluginable, SharedPluginable};

pub mod __internal {
    pub use crate::plugin::pluginable::{HookFuture, LocalHookFuture};
    pub use crate::plugin_driver::PluginDriver;
}
