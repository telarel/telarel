use oxc::ast::ast::{
    CommentContent, CommentKind, CommentNewlines, CommentPosition,
};

/// A partially adapted comment.
///
/// Adapters return only the fields they understand; the normalizer folds
/// several results with the first `Some` value winning per field.
#[derive(Debug, Clone, Default)]
pub struct AdaptedComment {
    /// The UTF-16 start offset.
    pub start: Option<u32>,
    /// The UTF-16 end offset.
    pub end: Option<u32>,
    /// The UTF-16 boundary this comment attaches to.
    pub attached_to: Option<u32>,
    /// The requested placement. Adapters validate the label; normalization
    /// discards it and forces leading because oxc codegen emits only leading
    /// comments.
    pub position: Option<CommentPosition>,
    /// The newline flags around the comment.
    pub newlines: Option<CommentNewlines>,
    /// The comment content classification.
    pub content: Option<CommentContent>,
}

/// A validated comment ready to attach to an oxc program.
#[derive(Debug, Clone)]
pub struct NormalizedComment {
    /// The byte start offset, delimiters included.
    pub start: u32,
    /// The byte end offset, delimiters included.
    pub end: u32,
    /// The byte boundary this comment attaches to.
    pub attached_to: u32,
    /// The kind derived from the source delimiters.
    pub kind: CommentKind,
    /// The effective placement; oxc 0.154 codegen only prints leading
    /// comments, so normalization always yields
    /// [`CommentPosition::Leading`].
    pub position: CommentPosition,
    /// The newline flags around the comment.
    pub newlines: CommentNewlines,
    /// The comment content classification.
    pub content: CommentContent,
}
