use bitflags::bitflags;

bitflags! {
    /// Which plugin hooks a plugin implements.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct HookUsage: u8 {
        /// The `options` hook.
        const Options = 1 << 0;
        /// The `pre` hook.
        const Pre = 1 << 1;
        /// The `transform` hook.
        const Transform = 1 << 2;
        /// The `post` hook.
        const Post = 1 << 3;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hook_usage_bits_are_distinct() {
        assert_ne!(HookUsage::Options, HookUsage::Pre);
        assert_ne!(HookUsage::Pre, HookUsage::Transform);
        assert_ne!(HookUsage::Transform, HookUsage::Post);
    }

    #[test]
    fn test_hook_usage_default_is_empty() {
        let usage: HookUsage = HookUsage::default();
        assert!(usage.is_empty());
    }

    #[test]
    fn test_hook_usage_contains_and_union() {
        let usage: HookUsage = HookUsage::Pre | HookUsage::Post;
        assert!(usage.contains(HookUsage::Pre));
        assert!(usage.contains(HookUsage::Post));
        assert!(!usage.contains(HookUsage::Transform));
    }
}
