/// The execution bucket a hook requests relative to the normal registration
/// order of the plugins that implement it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginOrder {
    /// Runs before every normal plugin for the hook.
    Pre,
    /// Runs after every normal plugin for the hook.
    Post,
}

/// Per-hook ordering metadata. A hook with no meta runs in the normal bucket.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PluginHookMeta {
    /// The requested bucket; `None` means the normal bucket.
    pub order: Option<PluginOrder>,
}
