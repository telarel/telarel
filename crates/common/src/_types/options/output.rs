/// Configuration that names the generated code's intended output path.
///
/// The compiler emits code in memory. `file` labels that output, so the
/// pipeline can record `map.file` and derive the default external source-map
/// URL.
#[derive(Debug, Clone, Default)]
pub struct OutputOptions {
    /// The intended path of the generated code.
    ///
    /// The compiler does not write the file. A non-empty value is used
    /// verbatim. `None` or an empty string is unset. The transform plugin
    /// derives an unset value in its options hook. Otherwise, the compiler
    /// uses the resolved input `file`.
    pub file: Option<String>,
}

/// Output code configuration after defaults are applied.
///
/// `file` is the configured `output.file`, the value derived by the transform
/// plugin, or the resolved input `file`.
#[derive(Debug, Clone)]
pub struct ResolvedOutputOptions {
    /// The file the generated code is intended for.
    ///
    /// The resolved path is recorded as `map.file` and derives the default
    /// `<file>.map` URL when no external source-map URL is configured.
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
