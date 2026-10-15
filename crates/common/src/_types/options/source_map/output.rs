/// Where the output source map is delivered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputSourceMapMode {
    /// Append `//# sourceMappingURL=data:...` to the code; `CodeResult.map = None`.
    Inline,
    /// Return the map separately; no comment (default).
    External,
    /// Append the comment and also return the map.
    Both,
}

/// Default mode when an output map is explicitly enabled without choosing a strategy.
impl Default for OutputSourceMapMode {
    fn default() -> Self {
        Self::External
    }
}

/// Options for the output source map.
#[derive(Debug, Clone, Default)]
pub struct OutputSourceMapOptions {
    /// How the map is delivered.
    pub mode: OutputSourceMapMode,
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
}
