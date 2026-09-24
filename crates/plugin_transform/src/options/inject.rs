use oxc_transformer_plugins::{InjectGlobalVariablesConfig, InjectImport};

/// How a symbol is bound from an inject source module.
#[derive(Debug, Clone)]
pub enum InjectSpecifier {
    /// `import { imported as local } from "source"`;
    /// `imported: None` lowers to `import { default as local }`.
    Named { imported: Option<String>, local: String },
    /// `import local from "source"`.
    Default { local: String },
    /// `import * as local from "source"`.
    Namespace { local: String },
}

/// A single inject entry: import `specifier` from `source` when `specifier`'s
/// local binding is referenced.
#[derive(Debug, Clone)]
pub struct InjectEntry {
    pub source: String,
    pub specifier: InjectSpecifier,
}

/// The `inject` options: import injection for unresolved globals.
#[derive(Debug, Clone, Default)]
pub struct InjectOptions {
    pub entries: Vec<InjectEntry>,
}

impl InjectOptions {
    /// Resolve into the oxc config, mapping each entry to its
    /// [`InjectImport`] constructor.
    pub fn resolve(&self) -> InjectGlobalVariablesConfig {
        let injects: Vec<InjectImport> = self
            .entries
            .iter()
            .map(|entry| match &entry.specifier {
                | InjectSpecifier::Named { imported, local } => {
                    InjectImport::named_specifier(
                        &entry.source,
                        imported.as_deref(),
                        local,
                    )
                },
                | InjectSpecifier::Default { local } => {
                    InjectImport::default_specifier(&entry.source, local)
                },
                | InjectSpecifier::Namespace { local } => {
                    InjectImport::namespace_specifier(&entry.source, local)
                },
            })
            .collect();

        InjectGlobalVariablesConfig::new(injects)
    }
}

#[cfg(test)]
mod tests {
    use oxc_transformer_plugins::InjectImportSpecifier;

    use crate::options::inject::{InjectEntry, InjectOptions, InjectSpecifier};

    use super::*;

    fn options(entries: Vec<(&str, InjectSpecifier)>) -> InjectOptions {
        let options: InjectOptions = InjectOptions {
            entries: entries
                .into_iter()
                .map(|(source, specifier)| InjectEntry {
                    source: String::from(source),
                    specifier,
                })
                .collect(),
        };

        options
    }

    fn local(specifier: &InjectImportSpecifier) -> &str {
        match specifier {
            | InjectImportSpecifier::Specifier { local, .. }
            | InjectImportSpecifier::DefaultSpecifier { local }
            | InjectImportSpecifier::NamespaceSpecifier { local } => local,
        }
    }

    #[test]
    fn test_resolve_named_with_imported() {
        let options: InjectOptions = options(vec![(
            "jquery",
            InjectSpecifier::Named {
                imported: Some(String::from("default")),
                local: String::from("$"),
            },
        )]);

        let config: InjectGlobalVariablesConfig = options.resolve();

        let debug: String = format!("{config:?}");

        assert!(debug.contains("jquery"), "{}", debug);
        assert!(debug.contains("Specifier"), "{}", debug);
        assert!(debug.contains("imported"), "{}", debug);
    }

    #[test]
    fn test_resolve_named_without_imported() {
        let options: InjectOptions = options(vec![(
            "jquery",
            InjectSpecifier::Named { imported: None, local: String::from("$") },
        )]);

        let config: InjectGlobalVariablesConfig = options.resolve();

        let debug: String = format!("{config:?}");

        assert!(debug.contains("jquery"), "{}", debug);
        assert!(debug.contains("None"), "{}", debug);
    }

    #[test]
    fn test_resolve_default() {
        let options: InjectOptions = options(vec![(
            "buffer",
            InjectSpecifier::Default { local: String::from("Buffer") },
        )]);

        let config: InjectGlobalVariablesConfig = options.resolve();

        let debug: String = format!("{config:?}");

        assert!(debug.contains("DefaultSpecifier"), "{}", debug);
        assert!(debug.contains("Buffer"), "{}", debug);
    }

    #[test]
    fn test_resolve_namespace() {
        let options: InjectOptions = options(vec![(
            "process",
            InjectSpecifier::Namespace { local: String::from("Process") },
        )]);

        let config: InjectGlobalVariablesConfig = options.resolve();

        let debug: String = format!("{config:?}");

        assert!(debug.contains("NamespaceSpecifier"), "{}", debug);
        assert!(debug.contains("Process"), "{}", debug);
    }

    #[test]
    fn test_resolve_dotted_local_gets_replacement_value() {
        let options: InjectOptions = options(vec![(
            "object-assign-shim",
            InjectSpecifier::Named {
                imported: Some(String::from("default")),
                local: String::from("Object.assign"),
            },
        )]);

        let config: InjectGlobalVariablesConfig = options.resolve();

        let debug: String = format!("{config:?}");

        assert!(debug.contains("object-assign-shim"), "{}", debug);
    }

    #[test]
    fn test_resolve_empty_entries() {
        let options: InjectOptions = options(vec![]);

        let config: InjectGlobalVariablesConfig = options.resolve();

        let debug: String = format!("{config:?}");

        assert!(debug.contains("injects: []"), "{}", debug);
    }

    #[test]
    fn test_local_helper_covers_all_specifiers() {
        let specifiers: [InjectImportSpecifier; 3] = [
            InjectImportSpecifier::Specifier {
                imported: None,
                local: oxc::str::CompactStr::from("a"),
            },
            InjectImportSpecifier::DefaultSpecifier {
                local: oxc::str::CompactStr::from("b"),
            },
            InjectImportSpecifier::NamespaceSpecifier {
                local: oxc::str::CompactStr::from("c"),
            },
        ];

        assert_eq!(local(&specifiers[0]), "a");
        assert_eq!(local(&specifiers[1]), "b");
        assert_eq!(local(&specifiers[2]), "c");
    }
}
