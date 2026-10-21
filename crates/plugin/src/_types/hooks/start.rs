/// Arguments for the `start` hook.
#[derive(Debug, Clone)]
pub struct StartArgs {
    /// Resolved options; read-only.
    pub options: telarel_common::ResolvedOptions,
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use telarel_common::{
        Language, ResolvedOptions, SourceMapOptions, SourceType,
    };

    use super::*;

    #[test]
    fn test_start_args_carry_resolved_options() {
        let args: StartArgs = StartArgs {
            options: ResolvedOptions {
                cwd: PathBuf::from("/repo"),
                file: String::from("a.ts"),
                language: Language::TS,
                source_type: SourceType::Module,
                source_map: SourceMapOptions::default(),
                plugins: vec![],
            },
        };

        assert_eq!(args.options.cwd, PathBuf::from("/repo"));
        assert_eq!(args.options.plugins.len(), 0);
    }
}
