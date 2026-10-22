use std::path::PathBuf;

use crate::sourcemap::SourceMap;

/// How an input source map is obtained for a parse step.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum InputSourceMapMode {
    /// A source map value provided directly.
    Value(SourceMap),
    /// Read a `//# sourceMappingURL=data:...` comment from the code.
    Inline,
    /// Read the configured external map file, or `file` with `.map` appended.
    ///
    /// The `?query`/`#hash` suffix is ignored when deriving the default path.
    External,
    /// Inline comment first, then the configured external map file, or `file`
    /// with `.map` appended.
    ///
    /// The `?query`/`#hash` suffix is ignored when deriving the default path,
    /// and missing external maps are silently ignored.
    Auto,
}

/// Default mode when an input map is explicitly enabled without choosing a strategy.
impl Default for InputSourceMapMode {
    fn default() -> Self {
        Self::Auto
    }
}

/// A full input source-map request.
///
/// `mode` selects the strategy; `path` overrides the external map location
/// for `External` and the `Auto` fallback.
#[derive(Debug, Clone, Default)]
pub struct InputSourceMapOptions {
    /// How the map is obtained.
    pub mode: InputSourceMapMode,
    /// Local filesystem path to an external map file.
    ///
    /// Absolute paths are used as-is; relative paths resolve against the
    /// source file's directory. Used by `External` and the `Auto` fallback;
    /// ignored by other modes.
    pub path: Option<PathBuf>,
}

/// The input source map request, either a bare mode or full options.
#[derive(Debug, Clone)]
pub enum InputSourceMap {
    /// A bare input mode.
    Mode(InputSourceMapMode),
    /// Full input options.
    Options(InputSourceMapOptions),
}

impl Default for InputSourceMap {
    fn default() -> Self {
        Self::Mode(InputSourceMapMode::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_source_map_mode_default_is_auto() {
        assert!(matches!(
            InputSourceMapMode::default(),
            InputSourceMapMode::Auto,
        ));
    }

    #[test]
    fn test_input_source_map_default_is_mode_auto() {
        assert!(matches!(
            InputSourceMap::default(),
            InputSourceMap::Mode(InputSourceMapMode::Auto),
        ));
    }

    #[test]
    fn test_input_source_map_options_default_is_auto_without_path() {
        let options: InputSourceMapOptions = InputSourceMapOptions::default();

        assert!(matches!(options.mode, InputSourceMapMode::Auto));
        assert!(options.path.is_none());
    }
}
