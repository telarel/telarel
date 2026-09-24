use oxc::diagnostics::Diagnostics;
use oxc_transformer_plugins::ReplaceGlobalDefinesConfig;

/// The `define` options: esbuild-style global constant replacements.
///
/// Entries are ordered pairs; the first entry for a duplicate key wins.
#[derive(Debug, Clone, Default)]
pub struct DefineOptions {
    pub entries: Vec<(String, String)>,
}

impl DefineOptions {
    /// Resolve into the oxc config, validating keys and values.
    ///
    /// The oxc constructor parses each key and value once for grammatical
    /// errors, so invalid entries surface here as [`Diagnostics`].
    pub fn resolve(&self) -> Result<ReplaceGlobalDefinesConfig, Diagnostics> {
        ReplaceGlobalDefinesConfig::new(&self.entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolve(
        entries: Vec<(&str, &str)>
    ) -> Result<ReplaceGlobalDefinesConfig, Diagnostics> {
        let options: DefineOptions = DefineOptions {
            entries: entries
                .into_iter()
                .map(|(key, value)| (String::from(key), String::from(value)))
                .collect(),
        };

        options.resolve()
    }

    #[test]
    fn test_resolve_identifier() {
        let config: ReplaceGlobalDefinesConfig =
            resolve(vec![("__DEV__", "false")]).expect("valid define");

        assert!(format!("{config:?}").contains("__DEV__"));
    }

    #[test]
    fn test_resolve_chain() {
        let config: ReplaceGlobalDefinesConfig =
            resolve(vec![("process.env.NODE_ENV", "\"production\"")])
                .expect("valid define");

        assert!(format!("{config:?}").contains("NODE_ENV"));
    }

    #[test]
    fn test_resolve_typeof() {
        let config: ReplaceGlobalDefinesConfig =
            resolve(vec![("typeof window", "\"object\"")])
                .expect("valid define");

        assert!(format!("{config:?}").contains("typeof"));
    }

    #[test]
    fn test_resolve_import_meta() {
        let config: ReplaceGlobalDefinesConfig = resolve(vec![
            ("import.meta.env.MODE", "\"development\""),
            ("import.meta.env.*", "undefined"),
        ])
        .expect("valid defines");

        let debug: String = format!("{config:?}");

        assert!(debug.contains("MODE"));
        assert!(debug.contains("wildcard"));
    }

    #[test]
    fn test_resolve_duplicate_key_first_match_wins() {
        let first: ReplaceGlobalDefinesConfig =
            resolve(vec![("__DEV__", "false"), ("__DEV__", "true")])
                .expect("valid defines");

        let second: ReplaceGlobalDefinesConfig =
            resolve(vec![("__DEV__", "true")]).expect("valid define");

        let first_debug: String = format!("{first:?}");
        let second_debug: String = format!("{second:?}");

        assert!(
            first_debug.contains("\"__DEV__\": \"false\""),
            "{}",
            first_debug
        );
        assert!(
            second_debug.contains("\"__DEV__\": \"true\""),
            "{}",
            second_debug
        );
    }

    #[test]
    fn test_resolve_invalid_key_errors() {
        let result: Result<ReplaceGlobalDefinesConfig, Diagnostics> =
            resolve(vec![("not an identifier", "1")]);

        let error: Diagnostics = result.expect_err("invalid key errors");

        assert!(format!("{error:?}").contains("not an identifier"));
    }

    #[test]
    fn test_resolve_invalid_value_errors() {
        let result: Result<ReplaceGlobalDefinesConfig, Diagnostics> =
            resolve(vec![("__DEV__", "not(")]);

        let error: Diagnostics = result.expect_err("invalid value errors");

        assert!(error.has_errors());
    }
}
