/// Where the output source map is delivered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputSourceMapMode {
    /// Append `//# sourceMappingURL=data:...` to the code; `CodeResult.map = None`.
    Inline,
    /// Return the map separately; append an external comment when a map URL is
    /// configured (default).
    External,
    /// Append the data-URI comment and also return the map.
    Both,
}

/// Default mode when an output map is explicitly enabled without choosing a strategy.
impl Default for OutputSourceMapMode {
    fn default() -> Self {
        Self::External
    }
}

/// A full output source-map request.
///
/// `mode` selects how the map is delivered; `url` overrides the appended
/// external URL for `External`.
#[derive(Debug, Clone, Default)]
pub struct OutputSourceMapOptions {
    /// How the map is delivered.
    pub mode: OutputSourceMapMode,
    /// URL written verbatim into the appended `sourceMappingURL` comment.
    ///
    /// It is neither resolved nor fetched; an empty string is treated as
    /// absent. Used only by `External`; ignored by other modes.
    pub url: Option<String>,
}

/// The output source map request, either a bare mode or full options.
#[derive(Debug, Clone)]
pub enum OutputSourceMap {
    /// A bare mode.
    Mode(OutputSourceMapMode),
    /// Full output options.
    Options(OutputSourceMapOptions),
}

impl Default for OutputSourceMap {
    fn default() -> Self {
        Self::Mode(OutputSourceMapMode::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_source_map_mode_default_is_external() {
        assert_eq!(
            OutputSourceMapMode::default(),
            OutputSourceMapMode::External,
        );
    }

    #[test]
    fn test_output_source_map_default_is_mode_external() {
        let output_map: OutputSourceMap = OutputSourceMap::default();

        assert!(matches!(
            output_map,
            OutputSourceMap::Mode(OutputSourceMapMode::External),
        ));
    }

    #[test]
    fn test_output_source_map_options_default_is_external_without_url() {
        let options: OutputSourceMapOptions = OutputSourceMapOptions::default();

        assert_eq!(options.mode, OutputSourceMapMode::External);
        assert!(options.url.is_none());
    }
}
