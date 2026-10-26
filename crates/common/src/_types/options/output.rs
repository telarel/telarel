/// Configuration that names the generated code's intended output path.
///
/// The compiler emits code in memory. `file` only labels that output, so the
/// default external source-map URL can be derived.
#[derive(Debug, Clone, Default)]
pub struct OutputOptions {
    /// The intended path of the generated code.
    ///
    /// The compiler does not write the file. When no external source-map URL
    /// is configured, the path derives the default `<file>.map` URL. `None`
    /// or an empty string falls back to the resolved `file`.
    pub file: Option<String>,
}

/// Output code configuration after defaults are applied.
///
/// `file` is the configured `output.file`, or the resolved input `file` when
/// that value is unset or empty.
#[derive(Debug, Clone)]
pub struct ResolvedOutputOptions {
    /// The file the generated code is intended for.
    pub file: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_options_default_has_no_file() {
        let options: OutputOptions = OutputOptions::default();

        assert!(options.file.is_none());
    }

    #[test]
    fn test_resolved_output_options_constructs() {
        let options: ResolvedOutputOptions =
            ResolvedOutputOptions { file: String::from("dist/index.js") };

        assert_eq!(options.file, "dist/index.js");
    }
}
