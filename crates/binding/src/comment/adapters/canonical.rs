use oxc::ast::ast::{CommentContent, CommentNewlines, CommentPosition};

use crate::comment::adapted::AdaptedComment;
use crate::comment::adapters::{CommentAdapter, content_from_label};
use crate::comment::normalize::CommentContext;
use crate::comment::raw::{RawComment, RawNewlines};

/// Reads Telarel's extended placement metadata: `attachedTo`, `position`,
/// `newlines`, and `content`.
pub struct CanonicalAdapter;

impl CommentAdapter for CanonicalAdapter {
    fn adapt(
        &self,
        raw: &RawComment,
        _ctx: &CommentContext<'_>,
    ) -> anyhow::Result<AdaptedComment> {
        let position: Option<CommentPosition> = match raw.position.as_deref() {
            | Some("leading") => Some(CommentPosition::Leading),
            | Some("trailing") => Some(CommentPosition::Trailing),
            | Some(other) => {
                anyhow::bail!("unsupported position \"{other}\"")
            },
            | None => None,
        };

        let newlines: Option<CommentNewlines> =
            raw.newlines.as_ref().map(|newlines: &RawNewlines| {
                let mut flags: CommentNewlines = CommentNewlines::None;

                if newlines.leading {
                    flags |= CommentNewlines::Leading;
                }

                if newlines.trailing {
                    flags |= CommentNewlines::Trailing;
                }

                flags
            });

        let content: Option<CommentContent> = match raw.content.as_deref() {
            | Some(label) => Some(content_from_label(label)?),
            | None => None,
        };

        Ok(AdaptedComment {
            attached_to: raw.attached_to,
            position,
            newlines,
            content,
            ..AdaptedComment::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use oxc::ast::ast::{CommentContent, CommentNewlines, CommentPosition};

    use crate::comment::adapted::AdaptedComment;
    use crate::comment::adapters::CommentAdapter;
    use crate::comment::adapters::canonical::CanonicalAdapter;
    use crate::comment::normalize::CommentContext;
    use crate::comment::raw::{RawComment, RawNewlines};
    use crate::plugin::spans::SpanMap;

    fn context<'a>(
        source: &'a str,
        spans: &'a SpanMap,
    ) -> CommentContext<'a> {
        CommentContext { source, spans, anchors: &[] }
    }

    #[test]
    fn test_canonical_reads_placement_metadata() {
        let raw: RawComment = RawComment {
            attached_to: Some(7),
            position: Some(String::from("trailing")),
            newlines: Some(RawNewlines { leading: true, trailing: false }),
            content: Some(String::from("legal")),
            ..RawComment::default()
        };

        let source: &str = "// x";
        let spans: SpanMap = SpanMap::new(source);
        let adapted: AdaptedComment = CanonicalAdapter
            .adapt(&raw, &context(source, &spans))
            .expect("adapts");

        assert_eq!(adapted.attached_to, Some(7));
        assert_eq!(adapted.position, Some(CommentPosition::Trailing));
        assert_eq!(adapted.newlines, Some(CommentNewlines::Leading));
        assert_eq!(adapted.content, Some(CommentContent::Legal));
        assert_eq!(adapted.start, None);
        assert_eq!(adapted.end, None);
    }

    #[test]
    fn test_canonical_defaults_missing_newline_flags_to_false() {
        let raw: RawComment = serde_json::from_str(
            r#"{"type":"Line","newlines":{"leading":true}}"#,
        )
        .expect("the raw comment parses");

        let source: &str = "// x";
        let spans: SpanMap = SpanMap::new(source);
        let adapted: AdaptedComment = CanonicalAdapter
            .adapt(&raw, &context(source, &spans))
            .expect("adapts");

        assert_eq!(adapted.newlines, Some(CommentNewlines::Leading));
    }

    #[test]
    fn test_canonical_rejects_unknown_labels() {
        let source: &str = "// x";
        let spans: SpanMap = SpanMap::new(source);

        let position: RawComment = RawComment {
            position: Some(String::from("middle")),
            ..RawComment::default()
        };

        assert!(
            CanonicalAdapter
                .adapt(&position, &context(source, &spans))
                .is_err()
        );

        let content: RawComment = RawComment {
            content: Some(String::from("mystery")),
            ..RawComment::default()
        };

        assert!(
            CanonicalAdapter.adapt(&content, &context(source, &spans)).is_err()
        );
    }
}
