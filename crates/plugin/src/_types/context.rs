use std::path::Path;

use telarel_common::{Language, SourceType};

/// Common plugin context: available to every hook, including `options`.
#[derive(Debug, Default)]
pub struct CommonPluginContext {}

/// Info about the module being compiled.
#[derive(Debug)]
pub struct ModuleInfo<'a> {
    /// The file being compiled.
    pub file: &'a str,
    /// The grammar of the code, resolved from options, the AST, or the file
    /// extension.
    pub language: Language,
    /// The module system of the code, resolved from the grammar.
    pub source_type: SourceType,
}

/// Plugin context for hooks that run after the `options` stage.
#[derive(Debug)]
pub struct PluginContext<'a> {
    /// Filesystem root used to resolve relative paths.
    pub cwd: &'a Path,
    /// Info about the module being compiled.
    pub module: ModuleInfo<'a>,
}

impl<'a> PluginContext<'a> {
    /// Create a plugin context from a cwd and module info.
    pub fn new(
        cwd: &'a Path,
        module: ModuleInfo<'a>,
    ) -> Self {
        Self { cwd, module }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

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
            language: Language::TS,
            source_type: SourceType::Module,
        };

        let ctx: PluginContext<'_> =
            PluginContext::new(Path::new("/repo"), module);

        assert_eq!(ctx.cwd, Path::new("/repo"));
        assert_eq!(ctx.module.file, "a.ts");
        assert_eq!(ctx.module.language, Language::TS);
        assert_eq!(ctx.module.source_type, SourceType::Module);
    }
}
