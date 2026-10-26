use crate::comment::adapted::AdaptedComment;
use crate::comment::adapters::CommentAdapter;
use crate::comment::normalize::CommentContext;
use crate::comment::raw::RawComment;

/// Reads standard `start`/`end` spans (Acorn, oxc, Esprima).
///
/// The adapter treats a lone `start` or `end` as no position at all, so a
/// lower-precedence adapter can take over.
pub struct StartEndAdapter;

impl CommentAdapter for StartEndAdapter {
    fn adapt(
        &self,
        raw: &RawComment,
        _ctx: &CommentContext<'_>,
    ) -> anyhow::Result<AdaptedComment> {
        match (raw.start, raw.end) {
            | (Some(start), Some(end)) => Ok(AdaptedComment {
                start: Some(start),
                end: Some(end),
                ..AdaptedComment::default()
            }),
            | _ => Ok(AdaptedComment::default()),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::comment::adapted::AdaptedComment;
    use crate::comment::adapters::CommentAdapter;
    use crate::comment::adapters::start_end::StartEndAdapter;
    use crate::comment::normalize::CommentContext;
    use crate::comment::raw::RawComment;
    use crate::plugin::spans::SpanMap;

    fn context<'a>(
        source: &'a str,
        spans: &'a SpanMap,
    ) -> CommentContext<'a> {
        CommentContext { source, spans, anchors: &[] }
    }

    #[test]
    fn test_start_end_reads_complete_span() {
        let source: &str = "// x";
        let spans: SpanMap = SpanMap::new(source);
        let raw: RawComment = RawComment {
            start: Some(0),
            end: Some(4),
            ..RawComment::default()
        };

        let adapted: AdaptedComment = StartEndAdapter
            .adapt(&raw, &context(source, &spans))
            .expect("adapts");

        assert_eq!(adapted.start, Some(0));
        assert_eq!(adapted.end, Some(4));
    }

    #[test]
    fn test_start_end_ignores_incomplete_span() {
        let source: &str = "// x";
        let spans: SpanMap = SpanMap::new(source);
        let start_only: RawComment =
            RawComment { start: Some(0), ..RawComment::default() };
        let end_only: RawComment =
            RawComment { end: Some(4), ..RawComment::default() };

        for raw in [start_only, end_only] {
            let adapted: AdaptedComment = StartEndAdapter
                .adapt(&raw, &context(source, &spans))
                .expect("adapts");

            assert_eq!(adapted.start, None);
            assert_eq!(adapted.end, None);
        }
    }
}
