pub mod compile;
pub mod plugin;

use serde::{Deserialize, Serialize};

/// A UTF-16 code-unit span in an error transport payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorSpanPayload {
    /// The inclusive start offset.
    pub start: u32,
    /// The exclusive end offset.
    pub end: u32,
}

/// A source location in an error transport payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorSourcePayload {
    /// The file the span refers to.
    pub file: String,
    /// The source text the span refers to.
    pub code: String,
    /// The UTF-16 span within `code`.
    pub span: ErrorSpanPayload,
}
