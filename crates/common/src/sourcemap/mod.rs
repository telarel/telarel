pub mod compose;
pub mod inline;
pub mod resolve;

use oxc_sourcemap::SourceMap as OxcSourceMap;

/// An owned source map crossing pipeline boundaries.
pub type SourceMap = OxcSourceMap<'static>;
