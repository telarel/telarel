use std::borrow::Cow;

use oxc_sourcemap::Token;

use crate::sourcemap::SourceMap;

/// Find the incremental token with the greatest dst position
/// `<= (line, col)`; `None` when the position precedes the map's first
/// token (the un-mapped head of the edit).
///
/// This is the `traceSegment`-style greatest-at-or-before lookup
/// (`SourceMap::lookup_token_approx` documents the same clamping as the
/// composition pattern); binary search over the dst-sorted slice.
fn incremental_entry_at(
    sorted: &[Token],
    line: u32,
    col: u32,
) -> Option<Token> {
    let idx: usize = sorted.partition_point(|token: &Token| {
        (token.get_dst_line(), token.get_dst_col()) <= (line, col)
    });

    if idx == 0 {
        return None;
    }

    Some(sorted[idx - 1])
}

/// Translate an upstream token's src position through an incremental map.
///
/// The upstream token's src position identifies a point in the code the
/// hook edit received; the incremental map's entry at (or before) that
/// point carries the local delta `(inc.src - inc.dst)` the edit applied
/// there, which is added onto the upstream position. Positions before the
/// incremental map's first token resolve nothing and pass through.
fn resolve_through_incremental(
    inc: &[Token],
    base_token: &Token,
) -> Token {
    incremental_entry_at(
        inc,
        base_token.get_src_line(),
        base_token.get_src_col(),
    )
    .map_or(*base_token, |inc_token: Token| {
        // src = base.src + (inc.src - inc.dst); an i64 floor guards
        // the arithmetic against a malformed pair without wrapping or
        // panicking (a well-formed edit's inc.src is >= inc.dst).
        let delta_line: i64 = i64::from(inc_token.get_src_line())
            - i64::from(inc_token.get_dst_line());

        let delta_col: i64 = i64::from(inc_token.get_src_col())
            - i64::from(inc_token.get_dst_col());

        let src_line: u32 =
            u32::try_from(i64::from(base_token.get_src_line()) + delta_line)
                .unwrap_or(base_token.get_src_line());

        let src_col: u32 =
            u32::try_from(i64::from(base_token.get_src_col()) + delta_col)
                .unwrap_or(base_token.get_src_col());

        Token::new(
            base_token.get_dst_line(),
            base_token.get_dst_col(),
            src_line,
            src_col,
            base_token.get_source_id(),
            base_token.get_name_id(),
        )
    })
}

/// Compose a hook-returned incremental map into its upstream map.
///
/// `upstream` maps the pipeline's generated positions (dst) to positions in
/// the ORIGINAL source (src); `incremental` is a hook-returned map: dst
/// positions in the code the hook received mapped back to src positions in
/// the code the hook produced. The composed result keeps `upstream`'s dst
/// positions and re-resolves every src position through `incremental`, so
/// the output maps generated positions directly back to the original
/// source.
///
/// The composed map keeps the upstream map's sources, contents, and names
/// — the incremental map contributes only position deltas. Tokens it
/// cannot resolve carry through unchanged.
fn compose_step(
    original_file: Option<&str>,
    upstream: &SourceMap,
    incremental: &SourceMap,
) -> SourceMap {
    // The incremental map, ordered by generated position so each upstream
    // token's src position resolves with one binary search.
    let mut inc: Vec<Token> = incremental.get_tokens().collect();

    inc.sort_by_key(|token: &Token| {
        (token.get_dst_line(), token.get_dst_col())
    });

    // The upstream map's tokens, ordered by generated position: oxc_sourcemap
    // requires the token slice to be dst-monotonic — `generate_lookup_table`
    // partitions the slice on dst-line transitions under that invariant, and
    // the VLQ encoder diffs every token's dst against the previously emitted
    // one. Composition keeps the upstream dst side verbatim, so emitting in
    // dst order satisfies the contract; the stable sort preserves the
    // upstream's own emission order within dst ties.
    let mut base: Vec<Token> = upstream.get_tokens().collect();

    base.sort_by_key(|token: &Token| {
        (token.get_dst_line(), token.get_dst_col())
    });

    let mut tokens: Vec<Token> = base
        .iter()
        .map(|base_token: &Token| resolve_through_incremental(&inc, base_token))
        .collect::<Vec<Token>>();

    // Two tokens at the same generated position would serialize to a
    // zero-length segment; oxc's builders collapse such collisions keeping
    // the FIRST token (the keep-first shape `ConcatSourceMapBuilder` applies
    // at map seams), so the composed list matches that here.
    tokens.dedup_by(|later: &mut Token, kept: &mut Token| {
        (later.get_dst_line(), later.get_dst_col())
            == (kept.get_dst_line(), kept.get_dst_col())
    });

    // The composed map keeps the upstream map's source identity (the
    // original source list is carried through); only src positions moved.
    // Sources/names/contents are re-owned so the result is `'static`.
    let file: Option<Cow<'static, str>> = original_file.map_or(
        upstream.get_file().map(str::to_owned).map(Cow::Owned),
        |file: &str| Some(Cow::Owned(String::from(file))),
    );

    let names: Vec<Cow<'static, str>> =
        upstream.get_names().map(str::to_owned).map(Cow::Owned).collect();

    let sources: Vec<Cow<'static, str>> =
        upstream.get_sources().map(str::to_owned).map(Cow::Owned).collect();

    let contents: Vec<Option<Cow<'static, str>>> = upstream
        .get_source_contents()
        .map(|content: Option<&str>| content.map(str::to_owned).map(Cow::Owned))
        .collect();

    SourceMap::new(
        file,
        names,
        upstream.get_source_root().map(str::to_owned).map(Cow::Owned),
        sources,
        contents,
        tokens.into_boxed_slice(),
        None,
    )
}

/// Compose a whole chain of hook-level incrementals onto the upstream
/// map, left-to-right.
pub fn compose_maps(
    original_file: Option<&str>,
    upstream: &SourceMap,
    incrementals: &[&SourceMap],
) -> SourceMap {
    incrementals.iter().fold(
        upstream.clone(),
        |acc: SourceMap, inc: &&SourceMap| {
            compose_step(original_file, &acc, inc)
        },
    )
}

#[cfg(test)]
mod tests {
    use oxc_sourcemap::SourceMapBuilder;

    use super::*;

    /// Build a map whose single source is `file` with `contents`, carrying
    /// `(dst_line, dst_col, src_line, src_col)` tokens.
    fn build_map(
        file: &str,
        contents: &str,
        tokens: &[(u32, u32, u32, u32)],
    ) -> SourceMap {
        let mut builder: SourceMapBuilder<'_> = SourceMapBuilder::default();

        builder.set_file(file);

        let source_id: u32 = builder.add_source_and_content(file, contents);

        for &(dst_line, dst_col, src_line, src_col) in tokens {
            builder.add_token(
                dst_line,
                dst_col,
                src_line,
                src_col,
                Some(source_id),
                None,
            );
        }

        builder.into_owned_sourcemap().into_inner()
    }

    fn src_positions(map: &SourceMap) -> Vec<(u32, u32, u32, u32)> {
        map.get_tokens()
            .map(|token: Token| {
                (
                    token.get_dst_line(),
                    token.get_dst_col(),
                    token.get_src_line(),
                    token.get_src_col(),
                )
            })
            .collect()
    }

    #[test]
    fn test_compose_translates_src_through_incremental() {
        // Upstream dst positions resolve into the edit-received code at
        // line 1; the edit shifted content down one line, so the
        // incremental entry dst(1, c) -> src(0, c) resolves those
        // positions back to line 0 (the original).
        let upstream: SourceMap =
            build_map("a.ts", "original", &[(0, 0, 1, 0), (0, 10, 1, 10)]);

        let incremental: SourceMap =
            build_map("a.ts", "edited", &[(1, 0, 0, 0), (1, 10, 0, 10)]);

        let composed: SourceMap =
            compose_maps(Some("a.ts"), &upstream, &[&incremental]);

        assert_eq!(
            src_positions(&composed),
            vec![(0, 0, 0, 0), (0, 10, 0, 10)],
        );
    }

    #[test]
    fn test_compose_applies_column_delta() {
        // The edit inserted a three-char prefix on the same line: the
        // incremental entry dst(0, 9) -> src(0, 6) resolves upstream
        // src column 9 back to column 6, dst unchanged.
        let upstream: SourceMap = build_map("a.ts", "orig", &[(0, 2, 0, 9)]);

        let incremental: SourceMap = build_map("a.ts", "edit", &[(0, 9, 0, 6)]);

        let composed: SourceMap =
            compose_maps(Some("a.ts"), &upstream, &[&incremental]);

        assert_eq!(src_positions(&composed), vec![(0, 2, 0, 6)],);
    }

    #[test]
    fn test_compose_unmapped_head_carries_through() {
        // The incremental map starts at dst col 5: upstream src positions
        // before it cannot resolve through the edit and carry through.
        let upstream: SourceMap =
            build_map("a.ts", "orig", &[(0, 0, 0, 2), (0, 1, 0, 9)]);

        let incremental: SourceMap = build_map("a.ts", "edit", &[(0, 5, 0, 0)]);

        let composed: SourceMap =
            compose_maps(Some("a.ts"), &upstream, &[&incremental]);

        // (0, 9) resolves through the entry (5 -> 0): col 9 - 5 = 4;
        // (0, 2) is in the unmapped head and stays.
        assert_eq!(src_positions(&composed), vec![(0, 0, 0, 2), (0, 1, 0, 4)],);
    }

    #[test]
    fn test_compose_empty_incremental_is_upstream() {
        let upstream: SourceMap = build_map("a.ts", "orig", &[(0, 0, 0, 0)]);

        let incremental: SourceMap = build_map("a.ts", "empty", &[]);

        // An empty incremental map resolves nothing: every token passes
        // through (the head rule treats every position as unmapped).
        let composed: SourceMap =
            compose_maps(Some("a.ts"), &upstream, &[&incremental]);

        assert_eq!(src_positions(&composed), src_positions(&upstream),);
    }

    #[test]
    fn test_compose_keeps_sources_names_and_contents() {
        // The upstream map has two tokens, the second named: composition
        // re-resolves positions but keeps sources/content/name ids.
        let mut builder: SourceMapBuilder<'_> = SourceMapBuilder::default();

        builder.set_file("a.ts");

        let source_id: u32 =
            builder.add_source_and_content("a.ts", "the original contents");

        let name_id: u32 = builder.add_name("the-name");

        builder.add_token(0, 0, 0, 0, Some(source_id), None);

        builder.add_token(0, 3, 0, 7, Some(source_id), Some(name_id));

        let upstream: SourceMap = builder.into_owned_sourcemap().into_inner();

        // The edit shifted everything by 10 columns from column 0.
        let incremental: SourceMap =
            build_map("a.ts", "edited", &[(0, 10, 0, 0)]);

        let composed: SourceMap =
            compose_maps(None, &upstream, &[&incremental]);

        assert_eq!(composed.get_source(0), Some("a.ts"));

        assert_eq!(
            composed.get_source_and_content(0),
            Some(("a.ts", "the original contents")),
        );

        let tokens: Vec<Token> = composed.get_tokens().collect();

        assert_eq!(tokens[1].get_name_id(), Some(0));

        assert_eq!(composed.get_name(0), Some("the-name"));

        // The entry (10 -> 0) resolves nothing below col 10: both tokens
        // carry through with positions intact.
        assert_eq!(src_positions(&composed), vec![(0, 0, 0, 0), (0, 3, 0, 7)],);
    }

    #[test]
    fn test_compose_chain_folds_left_to_right() {
        // Upstream resolves to line 0; the first edit shifts +1 line
        // (dst(1, 0) -> src(0, 4)) and the second shifts +2 total
        // (dst(2, 0) -> src(0, 4)). Folding left-to-right composes the
        // upstream src through BOTH edits.
        let upstream: SourceMap = build_map("a.ts", "orig", &[(0, 0, 0, 4)]);

        let to_b: SourceMap = build_map("a.ts", "b", &[(1, 0, 0, 4)]);

        let to_c: SourceMap = build_map("a.ts", "c", &[(2, 0, 0, 4)]);

        let composed: SourceMap =
            compose_maps(Some("a.ts"), &upstream, &[&to_b, &to_c]);

        assert_eq!(src_positions(&composed), vec![(0, 0, 0, 4)],);
    }

    #[test]
    fn test_compose_chain_with_empty_link() {
        // A chain where a middle hook omitted its map: the empty
        // incremental link resolves nothing and the remaining links still
        // apply.
        let upstream: SourceMap = build_map("a.ts", "orig", &[(0, 0, 1, 0)]);

        let first_edit: SourceMap = build_map("a.ts", "one", &[(1, 0, 0, 0)]);

        let incremental: SourceMap = build_map("a.ts", "empty", &[]);

        let composed: SourceMap =
            compose_maps(Some("a.ts"), &upstream, &[&first_edit, &incremental]);

        assert_eq!(src_positions(&composed), vec![(0, 0, 0, 0)],);
    }

    #[test]
    fn test_compose_reorder_edit_is_dst_monotonic_and_json_valid() {
        // A reorder-style edit moved the second statement above the first:
        // upstream dst positions follow OUTPUT order while src positions
        // still follow ORIGINAL order. oxc_sourcemap requires dst-monotonic
        // tokens (`to_json()` diffs against the previously EMITTED token;
        // `generate_lookup_table` partitions on dst-line transitions), so
        // emission must go through in dst order.
        let upstream: SourceMap = build_map(
            "a.ts",
            "b",
            &[(1, 0, 0, 20), (1, 8, 0, 0), (2, 0, 0, 10)],
        );

        let incremental: SourceMap = build_map("a.ts", "empty", &[]);

        let composed: SourceMap =
            compose_maps(Some("a.ts"), &upstream, &[&incremental]);

        // The emitted token list must be dst-ordered regardless of the src
        // order the old src-sorted emission produced.
        let dst_order: Vec<(u32, u32)> = composed
            .get_tokens()
            .map(|token: Token| (token.get_dst_line(), token.get_dst_col()))
            .collect();

        assert_eq!(
            dst_order,
            vec![(1, 0), (1, 8), (2, 0)],
            "composed tokens must be dst-ordered",
        );

        // Serialization must round-trip through oxc's own parser: the old
        // src-ordered emission produced a negative dst-line delta (its src
        // order ran `(2,0)` after `(1,8)` back into dst line 1), which the
        // encoder's unsigned dst-line diff corrupts.
        let json: String = composed.to_json_string();

        let round_tripped: SourceMap =
            SourceMap::from_json_string(Box::leak(Box::new(json)))
                .expect("to_json() must round-trip");

        let lookup_table: Vec<&[Token]> = composed.generate_lookup_table();

        let top: Option<Token> = composed.lookup_token(&lookup_table, 1, 8);

        let bottom: Option<Token> = composed.lookup_token(&lookup_table, 1, 0);

        assert_eq!(
            top.map(|token: Token| (token.get_src_line(), token.get_src_col())),
            Some((0, 0)),
            "the now-top statement resolves to its original position",
        );

        assert_eq!(
            bottom.map(|token: Token| (
                token.get_src_line(),
                token.get_src_col()
            )),
            Some((0, 20)),
            "the now-bottom statement resolves to its original position",
        );

        assert_eq!(
            round_tripped
                .get_tokens()
                .map(|token: Token| {
                    (
                        token.get_dst_line(),
                        token.get_dst_col(),
                        token.get_src_line(),
                        token.get_src_col(),
                    )
                })
                .collect::<Vec<(u32, u32, u32, u32)>>(),
            src_positions(&composed),
        );
    }
}
