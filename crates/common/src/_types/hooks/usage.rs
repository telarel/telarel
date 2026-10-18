use bitflags::bitflags;

bitflags! {
    /// Which plugin hooks a plugin implements.
    ///
    /// The bit positions follow pipeline execution order.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct HookUsage: u32 {
        /// The `options` hook.
        const Options = 1 << 0;
        /// The `start` hook.
        const Start = 1 << 1;
        /// The `parse` hook.
        const Parse = 1 << 2;
        /// The `prepare` hook.
        const Prepare = 1 << 3;
        /// The `transform` hook.
        const Transform = 1 << 4;
        /// The `finalize` hook.
        const Finalize = 1 << 5;
        /// The `generate` hook.
        const Generate = 1 << 6;
        /// The `end` hook.
        const End = 1 << 7;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hook_usage_bits_are_distinct() {
        assert_ne!(HookUsage::Options, HookUsage::Start);
        assert_ne!(HookUsage::Start, HookUsage::Parse);
        assert_ne!(HookUsage::Parse, HookUsage::Prepare);
        assert_ne!(HookUsage::Prepare, HookUsage::Transform);
        assert_ne!(HookUsage::Transform, HookUsage::Finalize);
        assert_ne!(HookUsage::Finalize, HookUsage::Generate);
        assert_ne!(HookUsage::Generate, HookUsage::End);
    }

    #[test]
    fn test_hook_usage_bits_follow_execution_order() {
        assert!(HookUsage::Options.bits() < HookUsage::Start.bits());
        assert!(HookUsage::Start.bits() < HookUsage::Parse.bits());
        assert!(HookUsage::Parse.bits() < HookUsage::Prepare.bits());
        assert!(HookUsage::Prepare.bits() < HookUsage::Transform.bits());
        assert!(HookUsage::Transform.bits() < HookUsage::Finalize.bits());
        assert!(HookUsage::Finalize.bits() < HookUsage::Generate.bits());
        assert!(HookUsage::Generate.bits() < HookUsage::End.bits());
    }

    #[test]
    fn test_hook_usage_default_is_empty() {
        let usage: HookUsage = HookUsage::default();
        assert!(usage.is_empty());
    }

    #[test]
    fn test_hook_usage_contains_and_union() {
        let usage: HookUsage = HookUsage::Prepare | HookUsage::Finalize;
        assert!(usage.contains(HookUsage::Prepare));
        assert!(usage.contains(HookUsage::Finalize));
        assert!(!usage.contains(HookUsage::Transform));
    }
}
