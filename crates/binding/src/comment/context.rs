use crate::_types::spans::SpanMap;

/// The source-dependent inputs that every adapter and the normalizer share.
pub struct CommentContext<'a> {
    /// The source text the comment spans refer to.
    pub source: &'a str,
    /// The byte to UTF-16 boundary table for `source`.
    pub spans: &'a SpanMap,
    /// Printable boundaries in UTF-16 units, sorted and unique.
    pub anchors: &'a [u32],
}
