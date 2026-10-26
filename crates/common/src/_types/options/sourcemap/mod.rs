pub mod input;
pub mod output;

use crate::_types::options::sourcemap::input::InputSourceMap;
use crate::_types::options::sourcemap::output::OutputSourceMap;

/// The source-map inputs and outputs requested for a step.
#[derive(Debug, Clone, Default)]
pub struct SourceMapOptions {
    /// The input source-map request.
    ///
    /// `None` disables input-map lookup.
    pub input: Option<InputSourceMap>,
    /// The output source-map request.
    ///
    /// `None` means `External` at use time.
    pub output: Option<OutputSourceMap>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_source_map_options_default_is_off() {
        let options: SourceMapOptions = SourceMapOptions::default();

        assert!(options.input.is_none());
        assert!(options.output.is_none());
    }
}
