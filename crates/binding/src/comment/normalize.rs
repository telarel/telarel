use oxc::ast::ast::{
    CommentContent, CommentKind, CommentNewlines, CommentPosition,
};

use crate::comment::adapted::{AdaptedComment, NormalizedComment};
use crate::comment::adapters::{ADAPTERS, CommentAdapter};
use crate::comment::classify::classify_content;
use crate::comment::raw::RawComment;
use crate::plugin::spans::SpanMap;

/// The source-dependent inputs shared by every adapter and the normalizer.
pub struct CommentContext<'a> {
    /// The source text the comment spans refer to.
    pub source: &'a str,
    /// The byte/UTF-16 boundary table for `source`.
    pub spans: &'a SpanMap,
    /// Printable boundaries in UTF-16 units, sorted and unique.
    pub anchors: &'a [u32],
}

fn is_line_terminator(character: char) -> bool {
    matches!(character, '\r' | '\n' | '\u{2028}' | '\u{2029}')
}

fn newline_before_content(characters: impl Iterator<Item = char>) -> bool {
    for character in characters {
        if is_line_terminator(character) {
            return true;
        }

        if !character.is_whitespace() {
            return false;
        }
    }

    false
}

fn derive_newlines(
    source: &str,
    start: u32,
    end: u32,
) -> CommentNewlines {
    let mut newlines: CommentNewlines = CommentNewlines::None;

    if newline_before_content(source[..start as usize].chars().rev()) {
        newlines |= CommentNewlines::Leading;
    }

    if newline_before_content(source[end as usize..].chars()) {
        newlines |= CommentNewlines::Trailing;
    }

    newlines
}

fn comment_label(
    index: usize,
    raw: &RawComment,
) -> String {
    match (raw.start, raw.end) {
        | (Some(start), Some(end)) => {
            format!("comment {index} (start {start}, end {end})")
        },
        | (Some(start), None) => format!("comment {index} (start {start})"),
        | (None, Some(end)) => format!("comment {index} (end {end})"),
        | (None, None) => format!("comment {index}"),
    }
}

fn fold_adapters(
    raw: &RawComment,
    ctx: &CommentContext<'_>,
    adapters: &[&dyn CommentAdapter],
) -> anyhow::Result<AdaptedComment> {
    let mut folded: AdaptedComment = AdaptedComment::default();

    for adapter in adapters {
        let adapted: AdaptedComment = adapter.adapt(raw, ctx)?;

        folded.start = folded.start.or(adapted.start);
        folded.end = folded.end.or(adapted.end);
        folded.attached_to = folded.attached_to.or(adapted.attached_to);
        folded.position = folded.position.or(adapted.position);
        folded.newlines = folded.newlines.or(adapted.newlines);
        folded.content = folded.content.or(adapted.content);
    }

    Ok(folded)
}

fn utf16_to_byte_strict(
    offset: u32,
    label: &str,
    ctx: &CommentContext<'_>,
) -> anyhow::Result<u32> {
    let byte: usize = ctx.spans.utf16_to_byte(offset as usize);

    // `SpanMap::utf16_to_byte` clamps, so only a round trip proves that the
    // requested offset is a real UTF-16 boundary.
    if byte > ctx.source.len()
        || !ctx.source.is_char_boundary(byte)
        || ctx.spans.byte_to_utf16(byte) != offset as usize
    {
        anyhow::bail!("{label} offset {offset} is not a valid UTF-16 boundary");
    }

    Ok(byte as u32)
}

fn derive_kind(
    source: &str,
    start: u32,
    end: u32,
) -> anyhow::Result<CommentKind> {
    let slice: &str = &source[start as usize..end as usize];

    if slice.starts_with("//") {
        return Ok(CommentKind::Line);
    }

    if slice.starts_with("<!--") {
        return Ok(CommentKind::HtmlOpen);
    }

    if slice.starts_with("-->") {
        return Ok(CommentKind::HtmlClose);
    }

    if slice.starts_with("/*") {
        // `*/` is ASCII, so a valid closing delimiter also guarantees that
        // `end - 2` (the block value end) is a UTF-8 char boundary.
        if slice.len() < 4 || !slice.ends_with("*/") {
            anyhow::bail!("span is missing the closing block delimiter");
        }

        let multiline: bool = slice.chars().any(|character: char| {
            matches!(character, '\r' | '\n' | '\u{2028}' | '\u{2029}')
        });

        return Ok(if multiline {
            CommentKind::MultiLineBlock
        } else {
            CommentKind::SingleLineBlock
        });
    }

    anyhow::bail!("span does not start with a comment delimiter")
}

fn is_line_kind(kind: CommentKind) -> bool {
    matches!(
        kind,
        CommentKind::Line | CommentKind::HtmlOpen | CommentKind::HtmlClose
    )
}

fn is_block_kind(kind: CommentKind) -> bool {
    matches!(kind, CommentKind::SingleLineBlock | CommentKind::MultiLineBlock)
}

fn validate_type(
    raw_type: Option<&str>,
    kind: CommentKind,
) -> anyhow::Result<()> {
    let Some(raw_type) = raw_type else {
        anyhow::bail!("missing type");
    };

    match raw_type {
        | "Line" if !is_line_kind(kind) => {
            anyhow::bail!("type \"Line\" does not match the source delimiter");
        },
        | "Block" if !is_block_kind(kind) => {
            anyhow::bail!("type \"Block\" does not match the source delimiter");
        },
        | "Line" | "Block" => Ok(()),
        | other => {
            anyhow::bail!(
                "unsupported type \"{other}\"; use \"Line\" or \"Block\""
            );
        },
    }
}

fn derive_value(
    source: &str,
    start: u32,
    end: u32,
    kind: CommentKind,
) -> anyhow::Result<&str> {
    let start: usize = start as usize;
    let end: usize = end as usize;

    let (value_start, value_end): (usize, usize) = match kind {
        | CommentKind::Line => (start + 2, end),
        | CommentKind::HtmlOpen => (start + 4, end),
        | CommentKind::HtmlClose => (start + 3, end),
        | CommentKind::SingleLineBlock | CommentKind::MultiLineBlock => {
            (start + 2, end.saturating_sub(2))
        },
    };

    if value_start > end || value_end < value_start {
        anyhow::bail!("span is too short for its delimiter");
    }

    if !source.is_char_boundary(value_start)
        || !source.is_char_boundary(value_end)
    {
        anyhow::bail!("span does not align with character boundaries");
    }

    Ok(&source[value_start..value_end])
}

fn resolve_attached_to(
    adapted: Option<u32>,
    end_utf16: u32,
    ctx: &CommentContext<'_>,
) -> anyhow::Result<u32> {
    let source_utf16_len: u32 =
        ctx.spans.byte_to_utf16(ctx.source.len()) as u32;

    let accepted: Option<u32> = adapted.filter(|&anchor: &u32| {
        anchor >= end_utf16
            && anchor <= source_utf16_len
            && (anchor == 0
                || anchor == source_utf16_len
                || ctx.anchors.binary_search(&anchor).is_ok())
    });

    let anchor: u32 = match accepted {
        | Some(anchor) => anchor,
        | None => {
            let index: usize =
                ctx.anchors.partition_point(|&value: &u32| value < end_utf16);

            ctx.anchors.get(index).copied().unwrap_or(source_utf16_len)
        },
    };

    utf16_to_byte_strict(anchor, "attachedTo", ctx)
}

fn normalize_comment(
    raw: &RawComment,
    ctx: &CommentContext<'_>,
    adapters: &[&dyn CommentAdapter],
) -> anyhow::Result<NormalizedComment> {
    let adapted: AdaptedComment = fold_adapters(raw, ctx, adapters)?;

    let (start_utf16, end_utf16): (u32, u32) =
        match (adapted.start, adapted.end) {
            | (Some(start), Some(end)) => (start, end),
            | _ => anyhow::bail!("missing start/end"),
        };

    let start: u32 = utf16_to_byte_strict(start_utf16, "start", ctx)?;
    let end: u32 = utf16_to_byte_strict(end_utf16, "end", ctx)?;

    if start > end {
        anyhow::bail!("start is after end");
    }

    let kind: CommentKind = derive_kind(ctx.source, start, end)?;

    validate_type(raw.kind.as_deref(), kind)?;

    let value: &str = derive_value(ctx.source, start, end, kind)?;

    let content: CommentContent =
        adapted.content.unwrap_or_else(|| classify_content(value, kind));

    let attached_to: u32 =
        resolve_attached_to(adapted.attached_to, end_utf16, ctx)?;

    let newlines: CommentNewlines = adapted
        .newlines
        .unwrap_or_else(|| derive_newlines(ctx.source, start, end));

    Ok(NormalizedComment {
        start,
        end,
        attached_to,
        kind,
        position: CommentPosition::Leading,
        newlines,
        content,
    })
}

/// Normalize raw comments with an explicit adapter registry.
pub fn normalize_with_adapters(
    raw: &[RawComment],
    ctx: &CommentContext<'_>,
    adapters: &[&dyn CommentAdapter],
) -> anyhow::Result<Vec<NormalizedComment>> {
    let mut comments: Vec<NormalizedComment> = Vec::with_capacity(raw.len());

    for (index, comment) in raw.iter().enumerate() {
        let normalized: NormalizedComment = normalize_comment(
            comment, ctx, adapters,
        )
        .map_err(|error: anyhow::Error| {
            anyhow::anyhow!("{}: {error}", comment_label(index, comment))
        })?;

        comments.push(normalized);
    }

    comments.sort_by_key(|comment: &NormalizedComment| comment.start);

    Ok(comments)
}

/// Normalize raw comments with the default adapter registry.
pub fn normalize(
    raw: &[RawComment],
    ctx: &CommentContext<'_>,
) -> anyhow::Result<Vec<NormalizedComment>> {
    normalize_with_adapters(raw, ctx, ADAPTERS)
}

#[cfg(test)]
mod tests {
    use oxc::ast::ast::{CommentContent, CommentNewlines, CommentPosition};

    use crate::comment::adapted::NormalizedComment;
    use crate::comment::adapters::{CANONICAL, CommentAdapter};
    use crate::comment::normalize::{
        CommentContext, normalize, normalize_with_adapters,
    };
    use crate::comment::raw::{RawComment, RawNewlines};
    use crate::plugin::spans::SpanMap;

    fn raw_line(
        start: u32,
        end: u32,
    ) -> RawComment {
        RawComment {
            kind: Some(String::from("Line")),
            start: Some(start),
            end: Some(end),
            ..RawComment::default()
        }
    }

    fn raw_block(
        start: u32,
        end: u32,
    ) -> RawComment {
        RawComment {
            kind: Some(String::from("Block")),
            start: Some(start),
            end: Some(end),
            ..RawComment::default()
        }
    }

    fn normalize_with(
        source: &str,
        raw: &[RawComment],
        anchors: &[u32],
    ) -> anyhow::Result<Vec<NormalizedComment>> {
        let spans: SpanMap = SpanMap::new(source);
        let ctx: CommentContext<'_> =
            CommentContext { source, spans: &spans, anchors };

        normalize(raw, &ctx)
    }

    fn utf16(
        spans: &SpanMap,
        byte: usize,
    ) -> u32 {
        spans.byte_to_utf16(byte) as u32
    }

    fn normalize_bytes(
        source: &str,
        start: usize,
        end: usize,
        anchors: &[u32],
    ) -> anyhow::Result<Vec<NormalizedComment>> {
        let spans: SpanMap = SpanMap::new(source);
        let raw: RawComment =
            raw_line(utf16(&spans, start), utf16(&spans, end));

        normalize_with(source, &[raw], anchors)
    }

    #[test]
    fn test_normalize_round_trips_utf16_spans() {
        let cases: [(&str, usize, usize, &[u32]); 3] = [
            ("// µ\nconst x = 1;", 0, 5, &[0, 5, 17]),
            ("// 日\nconst x = 1;", 0, 6, &[0, 5, 17]),
            ("// 😀\nconst x = 1;", 0, 7, &[0, 6, 18]),
        ];

        for (source, start, end, anchors) in cases {
            let comments: Vec<NormalizedComment> =
                normalize_bytes(source, start, end, anchors)
                    .expect("the comment normalizes");

            assert_eq!(comments.len(), 1);
            assert_eq!(comments[0].start, start as u32, "{source}");
            assert_eq!(comments[0].end, end as u32, "{source}");
        }
    }

    #[test]
    fn test_normalize_rejects_mid_surrogate_and_out_of_range_offsets() {
        let source: &str = "// 😀\nconst x = 1;";
        let anchors: [u32; 3] = [0, 6, 18];

        // The emoji occupies UTF-16 units 3..5; 4 is mid-surrogate.
        let error: anyhow::Error =
            normalize_with(source, &[raw_line(0, 4)], &anchors)
                .expect_err("mid-surrogate offsets are rejected");

        assert_eq!(
            error.to_string(),
            "comment 0 (start 0, end 4): end offset 4 is not a valid UTF-16 boundary"
        );

        let error: anyhow::Error =
            normalize_with(source, &[raw_line(0, 99)], &anchors)
                .expect_err("out-of-range offsets are rejected");

        assert!(
            error.to_string().contains("end offset 99 is not a valid"),
            "{error}"
        );
    }

    #[test]
    fn test_normalize_rejects_reversed_spans() {
        let source: &str = "// a\nconst x = 1;";
        let anchors: [u32; 3] = [0, 5, 16];
        let error: anyhow::Error =
            normalize_with(source, &[raw_line(4, 0)], &anchors)
                .expect_err("reversed spans are rejected");

        assert_eq!(
            error.to_string(),
            "comment 0 (start 4, end 0): start is after end"
        );
    }

    #[test]
    fn test_normalize_rejects_delimiter_type_mismatches() {
        let source: &str = "// a\n/* b */";
        let anchors: [u32; 3] = [0, 5, 12];

        let error: anyhow::Error =
            normalize_with(source, &[raw_block(0, 4)], &anchors)
                .expect_err("a line delimiter cannot be Block");

        assert!(
            error.to_string().contains("does not match the source delimiter"),
            "{error}"
        );

        let error: anyhow::Error =
            normalize_with(source, &[raw_line(5, 12)], &anchors)
                .expect_err("a block delimiter cannot be Line");

        assert!(
            error.to_string().contains("does not match the source delimiter"),
            "{error}"
        );
    }

    #[test]
    fn test_normalize_requires_a_type() {
        let source: &str = "// a";
        let anchors: [u32; 2] = [0, 4];
        let raw: RawComment = RawComment {
            start: Some(0),
            end: Some(4),
            ..RawComment::default()
        };
        let error: anyhow::Error = normalize_with(source, &[raw], &anchors)
            .expect_err("the type is required");

        assert_eq!(
            error.to_string(),
            "comment 0 (start 0, end 4): missing type"
        );
    }

    #[test]
    fn test_normalize_requires_a_complete_span() {
        let source: &str = "// a";
        let anchors: [u32; 2] = [0, 4];
        let raw: RawComment = RawComment {
            kind: Some(String::from("Line")),
            ..RawComment::default()
        };
        let error: anyhow::Error = normalize_with(source, &[raw], &anchors)
            .expect_err("the span is required");

        assert_eq!(error.to_string(), "comment 0: missing start/end");

        let lone_start: RawComment = RawComment {
            kind: Some(String::from("Line")),
            start: Some(0),
            ..RawComment::default()
        };
        let error: anyhow::Error =
            normalize_with(source, &[lone_start], &anchors)
                .expect_err("a lone endpoint is not a span");

        assert_eq!(error.to_string(), "comment 0 (start 0): missing start/end");
    }

    #[test]
    fn test_normalize_rejects_babel_type_names() {
        let source: &str = "// a";
        let anchors: [u32; 2] = [0, 4];

        for name in ["CommentLine", "CommentBlock"] {
            let raw: RawComment = RawComment {
                kind: Some(String::from(name)),
                start: Some(0),
                end: Some(4),
                ..RawComment::default()
            };

            let error: anyhow::Error = normalize_with(source, &[raw], &anchors)
                .expect_err("Babel type names are rejected");

            assert!(error.to_string().contains("unsupported type"), "{error}");
        }
    }

    #[test]
    fn test_normalize_rejects_invalid_position_and_content_labels() {
        let source: &str = "// a";
        let anchors: [u32; 2] = [0, 4];

        let mut positioned: RawComment = raw_line(0, 4);
        positioned.position = Some(String::from("middle"));
        let error: anyhow::Error =
            normalize_with(source, &[positioned], &anchors)
                .expect_err("an unknown position label is rejected");

        assert_eq!(
            error.to_string(),
            "comment 0 (start 0, end 4): unsupported position \"middle\""
        );

        let mut classified: RawComment = raw_line(0, 4);
        classified.content = Some(String::from("mystery"));
        let error: anyhow::Error =
            normalize_with(source, &[classified], &anchors)
                .expect_err("an unknown content label is rejected");

        assert_eq!(
            error.to_string(),
            "comment 0 (start 0, end 4): unsupported content \"mystery\""
        );
    }

    #[test]
    fn test_normalize_honors_a_valid_explicit_anchor() {
        let source: &str = "// a\nconst x = 1;";
        let anchors: [u32; 3] = [0, 5, 16];
        let mut raw: RawComment = raw_line(0, 4);
        raw.attached_to = Some(5);

        let comments: Vec<NormalizedComment> =
            normalize_with(source, &[raw], &anchors)
                .expect("the comment normalizes");

        assert_eq!(comments[0].attached_to, 5);
        assert_eq!(comments[0].position, CommentPosition::Leading);
    }

    #[test]
    fn test_normalize_rederives_stale_explicit_anchors() {
        let source: &str = "// a\nconst x = 1;";
        let anchors: [u32; 3] = [0, 5, 16];

        for stale in [2, 6, 99] {
            let mut raw: RawComment = raw_line(0, 4);
            raw.attached_to = Some(stale);

            let comments: Vec<NormalizedComment> =
                normalize_with(source, &[raw], &anchors)
                    .expect("malformed anchors re-anchor instead of failing");

            assert_eq!(comments[0].attached_to, 5, "stale anchor {stale}");
        }
    }

    #[test]
    fn test_normalize_reanchors_trailing_comments_as_leading() {
        let source: &str = "// a\nconst x = 1;";
        let anchors: [u32; 3] = [0, 5, 16];
        let mut raw: RawComment = raw_line(0, 4);
        raw.attached_to = Some(4);
        raw.position = Some(String::from("trailing"));

        let comments: Vec<NormalizedComment> =
            normalize_with(source, &[raw], &anchors)
                .expect("the comment normalizes");

        assert_eq!(comments[0].position, CommentPosition::Leading);
        assert_eq!(comments[0].attached_to, 5);
    }

    #[test]
    fn test_normalize_derives_newlines() {
        let source: &str = "// a\n/* b */\nconst x = 1;";
        let anchors: [u32; 3] = [0, 13, 25];
        let comments: Vec<NormalizedComment> = normalize_with(
            source,
            &[raw_line(0, 4), raw_block(5, 12)],
            &anchors,
        )
        .expect("the comments normalize");

        assert_eq!(comments[0].newlines, CommentNewlines::Trailing);
        assert_eq!(
            comments[1].newlines,
            CommentNewlines::Leading | CommentNewlines::Trailing
        );
    }

    #[test]
    fn test_normalize_uses_supplied_newlines() {
        let source: &str = "// a\n";
        let anchors: [u32; 2] = [0, 5];
        let mut raw: RawComment = raw_line(0, 4);
        raw.newlines = Some(RawNewlines { leading: true, trailing: false });

        let comments: Vec<NormalizedComment> =
            normalize_with(source, &[raw], &anchors)
                .expect("the comment normalizes");

        assert_eq!(comments[0].newlines, CommentNewlines::Leading);
    }

    #[test]
    fn test_normalize_derives_content_from_comment_text() {
        let source: &str =
            "/*! legal */\n/** docs */\n/* webpackChunkName */\nconst x = 1;";
        let anchors: [u32; 3] = [0, 48, 60];
        let comments: Vec<NormalizedComment> = normalize_with(
            source,
            &[raw_block(25, 47), raw_block(0, 12), raw_block(13, 24)],
            &anchors,
        )
        .expect("the comments normalize");

        assert_eq!(
            comments[0].content,
            CommentContent::Legal,
            "sorted first by start"
        );
        assert_eq!(comments[1].content, CommentContent::Jsdoc);
        assert_eq!(comments[2].content, CommentContent::Webpack);
    }

    #[test]
    fn test_normalize_uses_supplied_content() {
        let source: &str = "// plain\nconst x = 1;";
        let anchors: [u32; 3] = [0, 9, 21];
        let mut raw: RawComment = raw_line(0, 8);
        raw.content = Some(String::from("coverage-ignore"));

        let comments: Vec<NormalizedComment> =
            normalize_with(source, &[raw], &anchors)
                .expect("the comment normalizes");

        assert_eq!(comments[0].content, CommentContent::CoverageIgnore);
    }

    #[test]
    fn test_normalize_sorts_by_start() {
        let source: &str = "// a\n// b\nconst x = 1;";
        let anchors: [u32; 4] = [0, 5, 10, 22];
        let comments: Vec<NormalizedComment> =
            normalize_with(source, &[raw_line(5, 9), raw_line(0, 4)], &anchors)
                .expect("the comments normalize");

        assert_eq!(comments[0].start, 0);
        assert_eq!(comments[1].start, 5);
    }

    #[test]
    fn test_normalize_rejects_malformed_block_spans() {
        let cases: [(&str, u32); 3] = [
            // The UTF-16 end lands on a byte inside the 3-byte `日`; without a
            // closing-delimiter check the value slice panics.
            ("/*日", 3),
            ("/* unterminated", 15),
            ("/*/", 3),
        ];

        for (source, end_utf16) in cases {
            let anchors: [u32; 2] = [0, end_utf16];
            let error: anyhow::Error =
                normalize_with(source, &[raw_block(0, end_utf16)], &anchors)
                    .expect_err("malformed block spans error");

            assert!(
                error.to_string().contains("closing block delimiter"),
                "{source}: {error}"
            );
        }

        let source: &str = "/*日*/";
        let anchors: [u32; 2] = [0, 5];
        let comments: Vec<NormalizedComment> =
            normalize_with(source, &[raw_block(0, 5)], &anchors)
                .expect("a delimited multi-byte block normalizes");

        assert_eq!(comments[0].start, 0);
        assert_eq!(comments[0].end, 7);
    }

    #[test]
    fn test_registry_accepts_start_end_only_comments() {
        let source: &str = "// a";
        let anchors: [u32; 2] = [0, 4];
        let comments: Vec<NormalizedComment> =
            normalize_with(source, &[raw_line(0, 4)], &anchors)
                .expect("the default registry reads start/end");

        assert_eq!(comments.len(), 1);
    }

    #[test]
    fn test_reduced_registry_rejects_start_end_only_comments() {
        let source: &str = "// a";
        let anchors: [u32; 2] = [0, 4];
        let spans: SpanMap = SpanMap::new(source);
        let ctx: CommentContext<'_> =
            CommentContext { source, spans: &spans, anchors: &anchors };
        let adapters: [&dyn CommentAdapter; 1] = [&CANONICAL];

        let error: anyhow::Error =
            normalize_with_adapters(&[raw_line(0, 4)], &ctx, &adapters)
                .expect_err("dropping START_END removes span support");

        assert_eq!(
            error.to_string(),
            "comment 0 (start 0, end 4): missing start/end"
        );
    }
}
