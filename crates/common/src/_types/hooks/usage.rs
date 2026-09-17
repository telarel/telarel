use bitflags::bitflags;

bitflags! {
    /// Which plugin hooks a plugin implements.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct HookUsage: u8 {
        /// The `options` hook.
        const OPTIONS = 1 << 0;
        /// The `pre` hook.
        const PRE = 1 << 1;
        /// The `transform` hook.
        const TRANSFORM = 1 << 2;
        /// The `post` hook.
        const POST = 1 << 3;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hook_usage_bits_are_distinct() {
        assert_ne!(HookUsage::OPTIONS, HookUsage::PRE);
        assert_ne!(HookUsage::PRE, HookUsage::TRANSFORM);
        assert_ne!(HookUsage::TRANSFORM, HookUsage::POST);
    }

    #[test]
    fn test_hook_usage_default_is_empty() {
        let usage: HookUsage = HookUsage::default();
        assert!(usage.is_empty());
    }

    #[test]
    fn test_hook_usage_contains_and_union() {
        let usage: HookUsage = HookUsage::PRE | HookUsage::POST;
        assert!(usage.contains(HookUsage::PRE));
        assert!(usage.contains(HookUsage::POST));
        assert!(!usage.contains(HookUsage::TRANSFORM));
    }
}
