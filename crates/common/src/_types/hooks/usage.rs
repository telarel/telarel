use bitflags::bitflags;

bitflags! {
    /// Which plugin hooks a plugin implements.
    ///
    /// The bit positions should follow pipeline execution order.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct HookUsage: u8 {
        /// The `options` hook.
        const Options = 1 << 0;
        /// The `compile_start` hook.
        const CompileStart = 1 << 1;
        /// The `pre` hook.
        const Pre = 1 << 2;
        /// The `transform` hook.
        const Transform = 1 << 3;
        /// The `post` hook.
        const Post = 1 << 4;
        /// The `compile_end` hook.
        const CompileEnd = 1 << 5;
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
        assert_ne!(HookUsage::Post, HookUsage::CompileStart);
        assert_ne!(HookUsage::CompileStart, HookUsage::CompileEnd);
    }

    #[test]
    fn test_hook_usage_bits_follow_execution_order() {
        assert!(HookUsage::Options.bits() < HookUsage::CompileStart.bits());
        assert!(HookUsage::CompileStart.bits() < HookUsage::Pre.bits());
        assert!(HookUsage::Pre.bits() < HookUsage::Transform.bits());
        assert!(HookUsage::Transform.bits() < HookUsage::Post.bits());
        assert!(HookUsage::Post.bits() < HookUsage::CompileEnd.bits());
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
