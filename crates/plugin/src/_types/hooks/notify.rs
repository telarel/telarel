use crate::PluginError;

/// The notify hook return (`start` / `prepare` / `finalize` / `end`): `()`
/// or an error.
pub type NotifyReturn = Result<(), PluginError>;
