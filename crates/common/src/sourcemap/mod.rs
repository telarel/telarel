pub mod compose;

use oxc_sourcemap::SourceMap as OxcSourceMap;

/// An owned source map crossing pipeline boundaries.
pub type SourceMap = OxcSourceMap<'static>;
