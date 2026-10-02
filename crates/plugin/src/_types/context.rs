use telarel_common::{Language, PluginState, SourceType};

/// Common plugin context: available to every hook, including `options`.
#[derive(Debug, Default)]
pub struct CommonPluginContext {
    /// Shared cross-plugin ambient state for one compile run.
    pub state: PluginState,
}

/// Info about the module being compiled.
#[derive(Debug)]
pub struct ModuleInfo<'a> {
    /// The file to be compiled.
    pub file: &'a str,
    /// The code to be compiled.
    pub code: &'a str,
    /// The grammar of the code, inferred from the file extension.
    pub language: Language,
    /// The module system of the code, resolved from the grammar.
    pub source_type: SourceType,
}

/// Plugin context for hooks that run after the `options` stage.
#[derive(Debug)]
pub struct PluginContext<'a> {
    /// Current working directory.
    pub cwd: &'a str,
    /// Info about the module being compiled.
    pub module: ModuleInfo<'a>,
    /// Shared cross-plugin ambient state for one compile run.
    pub state: &'a PluginState,
}

impl<'a> PluginContext<'a> {
    /// Create a plugin context from the shared state, a cwd, and module info.
    pub fn new(
        state: &'a PluginState,
        cwd: &'a str,
        module: ModuleInfo<'a>,
    ) -> Self {
        Self { state, cwd, module }
    }
}

#[cfg(test)]
mod tests {
    use telarel_common::{Language, SourceType};

    use super::*;

    #[test]
    fn test_common_context_defaults_to_empty_state() {
        let ctx: CommonPluginContext = CommonPluginContext::default();

        assert!(ctx.state.is_empty());
    }

    #[test]
    fn test_context_carries_state_cwd_and_module() {
        let common: CommonPluginContext = CommonPluginContext::default();

        let module: ModuleInfo<'_> = ModuleInfo {
            file: "a.ts",
            code: "let a;",
            language: Language::TS,
            source_type: SourceType::Module,
        };

        let ctx: PluginContext<'_> =
            PluginContext::new(&common.state, "/repo", module);

        assert!(ctx.state.is_empty());
        assert_eq!(ctx.cwd, "/repo");
        assert_eq!(ctx.module.file, "a.ts");
        assert_eq!(ctx.module.code, "let a;");
        assert_eq!(ctx.module.language, Language::TS);
        assert_eq!(ctx.module.source_type, SourceType::Module);
    }

    #[test]
    fn test_context_state_shares_the_common_map() {
        let common: CommonPluginContext = CommonPluginContext::default();

        let module: ModuleInfo<'_> = ModuleInfo {
            file: "a.ts",
            code: "let a;",
            language: Language::TS,
            source_type: SourceType::Module,
        };

        let ctx: PluginContext<'_> =
            PluginContext::new(&common.state, "/repo", module);

        assert_eq!(
            std::ptr::from_ref(ctx.state),
            std::ptr::from_ref(&common.state)
        );
    }
}
