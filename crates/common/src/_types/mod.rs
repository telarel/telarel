pub mod grammar;
pub mod hooks;
pub mod options;

/// An owned source map crossing pipeline boundaries.
pub type SourceMap = oxc_sourcemap::SourceMap<'static>;
