use std::path::PathBuf;

use crate::sourcemap::SourceMap;

/// How an input source map is obtained for a parse step.
#[derive(Debug, Clone)]
pub enum InputSourceMapMode {
    /// A source map value provided directly.
    Value(SourceMap),
    /// Path to a map file. Relative paths resolve against `cwd`.
    Path(PathBuf),
    /// Read a `//# sourceMappingURL=data:...` comment from the code.
    Inline,
    /// Read `<cwd>/<file>.map` from disk.
    External,
    /// Inline comment first, then `<cwd>/<file>.map`;
    /// missing sources are silently ignored (best effort).
    Auto,
}

/// Default mode when an input map is explicitly enabled without choosing a strategy.
impl Default for InputSourceMapMode {
    fn default() -> Self {
        Self::Auto
    }
}

/// The input source map request: a bare mode.
#[derive(Debug, Clone)]
pub enum InputSourceMap {
    /// A bare input mode.
    Mode(InputSourceMapMode),
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
}
