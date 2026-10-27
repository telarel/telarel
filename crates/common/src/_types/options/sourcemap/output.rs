/// Where the output source map is delivered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputSourceMapMode {
    /// Append `//# sourceMappingURL=data:...` to the code and set
    /// `CodeResult.map = None`.
    Inline,
    /// Return the map separately. Append an external comment when a map URL is
    /// configured. This is the default.
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
/// `mode` selects how the map is delivered. `url` overrides the appended
/// external URL for `External`.
#[derive(Debug, Clone, Default)]
pub struct OutputSourceMapOptions {
    /// How the map is delivered.
    pub mode: OutputSourceMapMode,
    /// URL written verbatim into the appended `sourceMappingURL` comment.
    ///
    /// The compiler neither resolves nor fetches this URL. An empty string is
    /// treated as absent. The field applies only to `External`, and other
    /// modes ignore it.
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

impl OutputSourceMap {
    /// Return the output mode for this output map.
    pub(crate) fn mode(&self) -> OutputSourceMapMode {
        match self {
            | Self::Mode(mode) => *mode,
            | Self::Options(options) => options.mode,
        }
    }

    /// Return the configured external map URL for this output map, if any.
    pub(crate) fn url(&self) -> Option<&str> {
        match self {
            | Self::Mode(_) => None,
            | Self::Options(options) => options.url.as_deref(),
        }
    }
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

    #[test]
    fn test_output_source_map_mode_reads_both_variants() {
        assert_eq!(
            OutputSourceMap::Mode(OutputSourceMapMode::Both).mode(),
            OutputSourceMapMode::Both,
        );
        assert_eq!(
            OutputSourceMap::Options(OutputSourceMapOptions {
                mode: OutputSourceMapMode::Inline,
                url: None,
            })
            .mode(),
            OutputSourceMapMode::Inline,
        );
    }

    #[test]
    fn test_output_source_map_url_is_none_for_bare_mode() {
        assert_eq!(
            OutputSourceMap::Mode(OutputSourceMapMode::External).url(),
            None,
        );
    }

    #[test]
    fn test_output_source_map_url_is_none_when_unset() {
        assert_eq!(
            OutputSourceMap::Options(OutputSourceMapOptions {
                mode: OutputSourceMapMode::External,
                url: None,
            })
            .url(),
            None,
        );
    }

    #[test]
    fn test_output_source_map_url_returns_configured_value() {
        let output: OutputSourceMap =
            OutputSourceMap::Options(OutputSourceMapOptions {
                mode: OutputSourceMapMode::External,
                url: Some(String::from("dist/app.js.map")),
            });

        assert_eq!(output.url(), Some("dist/app.js.map"));
    }

    #[test]
    fn test_output_source_map_url_returns_empty_value_verbatim() {
        let output: OutputSourceMap =
            OutputSourceMap::Options(OutputSourceMapOptions {
                mode: OutputSourceMapMode::External,
                url: Some(String::new()),
            });

        assert_eq!(output.url(), Some(""));
    }
}
