/// Raw newline flags around a comment; a missing flag is `false`.
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(default)]
pub struct RawNewlines {
    /// Whether a newline precedes the comment.
    pub leading: bool,
    /// Whether a newline follows the comment.
    pub trailing: bool,
}

/// A raw comment as supplied across the JS boundary.
///
/// Every field is optional because the accepted shapes vary by producer;
/// unknown keys (for example ESLint extras) are ignored.
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawComment {
    /// The comment type: `"Line"` or `"Block"`.
    #[serde(rename = "type")]
    pub kind: Option<String>,
    /// The UTF-16 start offset.
    pub start: Option<u32>,
    /// The UTF-16 end offset.
    pub end: Option<u32>,
    /// The UTF-16 boundary this comment attaches to.
    pub attached_to: Option<u32>,
    /// The placement label: `"leading"` or `"trailing"`.
    pub position: Option<String>,
    /// The newline flags around the comment.
    pub newlines: Option<RawNewlines>,
    /// The content classification label.
    pub content: Option<String>,
}
