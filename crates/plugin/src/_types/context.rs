use telarel_common::{Language, SourceType};

/// Common plugin context: available to every hook, including `options`.
#[derive(Debug, Default)]
pub struct CommonPluginContext {}

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
}

impl<'a> PluginContext<'a> {
    /// Create a plugin context from a cwd and module info.
    pub fn new(
        cwd: &'a str,
        module: ModuleInfo<'a>,
    ) -> Self {
        Self { cwd, module }
    }
}

#[cfg(test)]
mod tests {
    use telarel_common::{Language, SourceType};

    use super::*;

    #[test]
    fn test_common_context_defaults() {
        let ctx: CommonPluginContext = CommonPluginContext::default();

        assert_eq!(std::mem::size_of_val(&ctx), 0);
    }

    #[test]
    fn test_context_carries_cwd_and_module() {
        let module: ModuleInfo<'_> = ModuleInfo {
            file: "a.ts",
            code: "let a;",
            language: Language::TS,
            source_type: SourceType::Module,
        };

        let ctx: PluginContext<'_> = PluginContext::new("/repo", module);

        assert_eq!(ctx.cwd, "/repo");
        assert_eq!(ctx.module.file, "a.ts");
        assert_eq!(ctx.module.code, "let a;");
        assert_eq!(ctx.module.language, Language::TS);
        assert_eq!(ctx.module.source_type, SourceType::Module);
    }
}
