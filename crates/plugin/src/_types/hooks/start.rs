/// Arguments for the `start` hook.
#[derive(Debug, Clone)]
pub struct StartArgs {
    /// Resolved options; read-only.
    pub options: telarel_common::ResolvedOptions,
}

#[cfg(test)]
mod tests {
    use telarel_common::{Language, ResolvedOptions, SourceType};

    use super::*;

    #[test]
    fn test_start_args_carry_resolved_options() {
        let args: StartArgs = StartArgs {
            options: ResolvedOptions {
                cwd: String::from("/repo"),
                file: String::from("a.ts"),
                code: String::from("let a;"),
                language: Language::TS,
                source_type: SourceType::Module,
                plugins: vec![],
            },
        };

        assert_eq!(args.options.cwd, "/repo");
        assert_eq!(args.options.plugins.len(), 0);
    }
}
