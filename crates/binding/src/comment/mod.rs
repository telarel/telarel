//! Comment preservation across the JS boundary.
//!
//! The oxc ESTree codec skips `Program.comments`. This module serializes the
//! comments into the outbound program JSON, normalizes them on the inbound
//! side, and attaches them to the rebuilt program's arena.

pub mod adapted;
pub mod adapters;
pub mod classify;
pub mod normalize;
pub mod raw;
mod scan;

use oxc::ast::ast::{
    Comment, CommentContent, CommentPosition, Directive, Program, Statement,
};
use oxc::ast_visit::{Visit, walk};
use oxc::span::GetSpan as _;

use telarel_common::Ast;

use crate::comment::adapted::NormalizedComment;
use crate::comment::adapters::content_label;
use crate::comment::normalize::{CommentContext, normalize};
use crate::comment::raw::RawComment;
use crate::plugin::spans::SpanMap;

#[derive(serde::Serialize)]
struct JsonNewlines {
    leading: bool,
    trailing: bool,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonComment<'a> {
    #[serde(rename = "type")]
    kind: &'a str,
    value: &'a str,
    start: u32,
    end: u32,
    attached_to: u32,
    position: &'a str,
    newlines: JsonNewlines,
    content: &'a str,
}

struct PrintableStarts {
    starts: Vec<u32>,
}

impl<'a> Visit<'a> for PrintableStarts {
    fn visit_statement(
        &mut self,
        statement: &Statement<'a>,
    ) {
        if !matches!(
            statement,
            Statement::BlockStatement(_) | Statement::EmptyStatement(_)
        ) {
            self.starts.push(statement.span().start);
        }

        walk::walk_statement(self, statement);
    }

    fn visit_directive(
        &mut self,
        directive: &Directive<'a>,
    ) {
        self.starts.push(directive.span.start);

        walk::walk_directive(self, directive);
    }
}

fn is_stashed_annotation(comment: &Comment) -> bool {
    matches!(
        comment.content,
        CommentContent::Pure | CommentContent::NoSideEffects
    )
}

fn is_safe_anchor(
    comment: &Comment,
    starts: &[u32],
) -> bool {
    comment.is_leading()
        && comment.attached_to >= comment.span.end
        && starts.binary_search(&comment.attached_to).is_ok()
}

/// The guaranteed comment keys for codegen in a program, sorted and
/// deduplicated.
fn printable_starts(program: &Program<'_>) -> Vec<u32> {
    let mut visitor: PrintableStarts =
        PrintableStarts { starts: vec![0, program.span.end] };

    visitor.visit_program(program);

    visitor.starts.sort_unstable();
    visitor.starts.dedup();

    visitor.starts
}

fn needs_reanchor(program: &Program<'_>) -> bool {
    if program.comments.is_empty() {
        return false;
    }

    let starts: Vec<u32> = printable_starts(program);

    program.comments.iter().any(|comment: &Comment| {
        !is_stashed_annotation(comment) && !is_safe_anchor(comment, &starts)
    })
}

/// Re-anchor comments to `starts`, which must be sorted and include the
/// program start and end.
fn reanchor_with_starts(
    program: &mut Program<'_>,
    starts: &[u32],
) -> bool {
    let fallback: u32 = program.span.end;

    let mut changed: bool = false;

    for comment in &mut program.comments {
        if is_stashed_annotation(comment) {
            continue;
        }

        if !comment.is_leading() {
            comment.position = CommentPosition::Leading;
            changed = true;
        }

        if is_safe_anchor(comment, starts) {
            continue;
        }

        let index: usize =
            starts.partition_point(|&start: &u32| start < comment.span.end);

        let anchor: u32 = starts.get(index).copied().unwrap_or(fallback);

        if comment.attached_to != anchor {
            comment.attached_to = anchor;
            changed = true;
        }
    }

    changed
}

fn reanchor(program: &mut Program<'_>) -> bool {
    let starts: Vec<u32> = printable_starts(program);
    reanchor_with_starts(program, &starts)
}

fn json_comment<'a>(
    comment: &Comment,
    source: &'a str,
    spans: &SpanMap,
) -> JsonComment<'a> {
    JsonComment {
        kind: if comment.is_line() { "Line" } else { "Block" },
        value: comment.content_span().source_text(source),
        start: spans.byte_to_utf16(comment.span.start as usize) as u32,
        end: spans.byte_to_utf16(comment.span.end as usize) as u32,
        attached_to: spans.byte_to_utf16(comment.attached_to as usize) as u32,
        position: match comment.position {
            | oxc::ast::ast::CommentPosition::Leading => "leading",
            | oxc::ast::ast::CommentPosition::Trailing => "trailing",
        },
        newlines: JsonNewlines {
            leading: comment.preceded_by_newline(),
            trailing: comment.followed_by_newline(),
        },
        content: content_label(comment.content),
    }
}

/// Serialize `program.comments` into `program_json` as its first key.
///
/// The function returns the input unchanged when there are no comments, so
/// no-comment output stays byte-identical. `program_json` already carries
/// UTF-16 spans.
pub fn inject(
    ast: &Ast,
    spans: &SpanMap,
    program_json: String,
) -> String {
    let comments: &[Comment] = &ast.program().comments;

    if comments.is_empty() {
        return program_json;
    }

    let serialized: Vec<JsonComment<'_>> = comments
        .iter()
        .map(|comment: &Comment| json_comment(comment, ast.source(), spans))
        .collect();

    let array: String = serde_json::to_string(&serialized)
        .expect("comment JSON serialization cannot fail");

    let Some(open_index): Option<usize> = program_json.find('{') else {
        return program_json;
    };

    let (prefix, body): (&str, &str) = program_json.split_at(open_index + 1);

    let separator: &str =
        if body.trim_start().starts_with('}') { "" } else { "," };

    format!("{prefix}\"comments\":{array}{separator}{body}")
}

/// Whether `program_json` can contain a top-level `comments` key.
///
/// The literal check keeps the no-comments path cheap. An escaped key such as
/// `"\u0063omments"` cannot match the literal, so any Unicode escape also
/// triggers the scanner. ESTree output escapes keys rarely.
fn may_contain_comments(program_json: &str) -> bool {
    program_json.contains("\"comments\"") || program_json.contains("\\u")
}

/// Deserialize `program.comments` from program JSON with UTF-16 spans.
///
/// Only the top-level `comments` value is parsed. An absent key means no
/// comments. A present non-array value is an error. An entry that cannot be
/// deserialized is also an error.
pub fn extract_raw(program_json: &str) -> anyhow::Result<Vec<RawComment>> {
    if !may_contain_comments(program_json) {
        return Ok(Vec::new());
    }

    let Some(slice): Option<&str> = scan::comments_value(program_json)? else {
        return Ok(Vec::new());
    };

    let values: Vec<serde_json::Value> = serde_json::from_str(slice)?;

    let mut raw: Vec<RawComment> = Vec::with_capacity(values.len());

    for (index, value) in values.into_iter().enumerate() {
        let parsed: RawComment = serde_json::from_value(value).map_err(
            |error: serde_json::Error| {
                anyhow::anyhow!("comment {index}: {error}")
            },
        )?;

        raw.push(parsed);
    }

    Ok(raw)
}

/// Normalize raw comments against the rebuilt program's printable starts.
///
/// Returns the normalized comments and the byte starts they were anchored to,
/// so the caller can re-anchor without walking the program again.
pub fn normalize_from_program(
    raw: &[RawComment],
    program: &Program<'_>,
    source: &str,
    spans: &SpanMap,
) -> anyhow::Result<(Vec<NormalizedComment>, Vec<u32>)> {
    let starts: Vec<u32> = printable_starts(program);

    let mut anchors: Vec<u32> = starts
        .iter()
        .map(|start: &u32| spans.byte_to_utf16(*start as usize) as u32)
        .collect();

    anchors.sort_unstable();
    anchors.dedup();

    let ctx: CommentContext<'_> =
        CommentContext { source, spans, anchors: &anchors };

    let comments: Vec<NormalizedComment> = normalize(raw, &ctx)?;

    Ok((comments, starts))
}

/// Replace `program.comments` with `comments` and re-anchor them to `starts`.
///
/// `starts` must be sorted and include the program start and end. Duplicate
/// starts are tolerated.
pub fn attach_with_starts<'a>(
    allocator: &'a oxc::allocator::Allocator,
    program: &mut Program<'a>,
    comments: Vec<NormalizedComment>,
    starts: &[u32],
) {
    let mut arena: oxc::allocator::Vec<'a, Comment> =
        oxc::allocator::Vec::with_capacity_in(comments.len(), &allocator);

    for mut normalized in comments {
        // oxc diverts these annotations to a stash that the ESTree codec
        // cannot restore, so they become normal comments.
        if matches!(
            normalized.content,
            CommentContent::Pure | CommentContent::NoSideEffects
        ) {
            normalized.content = CommentContent::None;
        }

        let mut comment: Comment =
            Comment::new(normalized.start, normalized.end, normalized.kind);

        comment.attached_to = normalized.attached_to;
        comment.position = normalized.position;
        comment.newlines = normalized.newlines;
        comment.content = normalized.content;

        arena.push(comment);
    }

    program.comments = arena;

    reanchor_with_starts(program, starts);
}

/// Re-anchor comments on an existing AST to the starts that codegen prints at.
///
/// The function returns whether the AST changed. It skips pure and
/// no-side-effects annotations so the pass-through path keeps oxc's own
/// annotation stash.
pub fn reanchor_ast(ast: &mut Ast) -> bool {
    if !needs_reanchor(ast.program()) {
        return false;
    }

    ast.with_mut(
        |_allocator: &oxc::allocator::Allocator,
         program: &mut Program<'_>|
         -> bool { reanchor(program) },
    )
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use oxc::ast::ast::{
        Comment, CommentContent, CommentKind, CommentNewlines, CommentPosition,
    };
    use telarel_common::{
        Ast, GenerateCodeOptions, Language, ParseAstOptions, SourceType,
        generate_code, parse_ast,
    };

    use crate::ast::estree::{ast_from_program, ast_to_estree_json};
    use crate::ast::resolve::source_type_for_generate;
    use crate::comment::adapted::NormalizedComment;
    use crate::comment::raw::RawComment;
    use crate::comment::{
        attach_with_starts, extract_raw, inject, normalize_from_program,
        printable_starts, reanchor_ast,
    };
    use crate::plugin::spans::SpanMap;

    fn attach<'a>(
        allocator: &'a oxc::allocator::Allocator,
        program: &mut oxc::ast::ast::Program<'a>,
        comments: Vec<NormalizedComment>,
    ) {
        let starts: Vec<u32> = printable_starts(program);

        attach_with_starts(allocator, program, comments, &starts);
    }

    fn fixture_ast(source: &str) -> Ast {
        parse_ast(
            source.to_owned(),
            ParseAstOptions {
                cwd: PathBuf::from("/repo"),
                file: String::from("a.js"),
                language: Some(Language::JS),
                source_type: Some(SourceType::Script),
                input_map: None,
            },
        )
        .expect("fixture parses")
    }

    fn rebuilt_without_comments(source: &str) -> Ast {
        let ast: Ast = fixture_ast(source);
        let mut program: serde_json::Value =
            serde_json::from_str(&ast_to_estree_json(&ast))
                .expect("the serialized program parses");

        program
            .as_object_mut()
            .expect("the program is an object")
            .remove("comments");

        ast_from_program(
            &program,
            source.to_owned(),
            None,
            ast.program().source_type,
        )
        .expect("the rebuilt program decodes")
    }

    fn attach_one(
        ast: &mut Ast,
        comment: NormalizedComment,
    ) {
        ast.with_mut(
            |allocator: &oxc::allocator::Allocator,
             program: &mut oxc::ast::ast::Program<'_>| {
                attach(allocator, program, vec![comment]);
            },
        );
    }

    fn generated(ast: &Ast) -> String {
        generate_code(
            ast,
            GenerateCodeOptions { file: String::from("index.js") },
        )
        .code
    }

    fn raw_comments(program_json: &str) -> Vec<RawComment> {
        extract_raw(program_json).expect("the raw comments extract")
    }

    fn empty_ast(source: &str) -> Ast {
        let end: u32 = source.encode_utf16().count() as u32;
        let program: serde_json::Value = serde_json::json!({
            "type": "Program",
            "sourceType": "script",
            "body": [],
            "start": 0,
            "end": end,
        });

        ast_from_program(
            &program,
            source.to_string(),
            None,
            source_type_for_generate(Language::JS),
        )
        .expect("the empty program decodes")
    }

    fn normalized_line(
        start: u32,
        end: u32,
        attached_to: u32,
    ) -> NormalizedComment {
        NormalizedComment {
            start,
            end,
            attached_to,
            kind: CommentKind::Line,
            position: CommentPosition::Leading,
            newlines: CommentNewlines::None,
            content: CommentContent::None,
        }
    }

    fn normalized_block(
        start: u32,
        end: u32,
        attached_to: u32,
    ) -> NormalizedComment {
        NormalizedComment {
            start,
            end,
            attached_to,
            kind: CommentKind::SingleLineBlock,
            position: CommentPosition::Leading,
            newlines: CommentNewlines::None,
            content: CommentContent::None,
        }
    }

    #[test]
    fn test_inject_without_comments_is_byte_identical() {
        let ast: Ast = empty_ast("");
        let spans: SpanMap = SpanMap::new(ast.source());
        let json: String = String::from(r#"{"type":"Program","body":[]}"#);

        assert_eq!(inject(&ast, &spans, json.clone()), json);
    }

    #[test]
    fn test_inject_into_an_empty_object() {
        let source: &str = "// a";
        let mut ast: Ast = empty_ast(source);

        ast.with_mut(
            |allocator: &oxc::allocator::Allocator,
             program: &mut oxc::ast::ast::Program<'_>| {
                attach(allocator, program, vec![normalized_line(0, 4, 4)]);
            },
        );

        let spans: SpanMap = SpanMap::new(source);
        let json: String = inject(&ast, &spans, String::from("{}"));

        let value: serde_json::Value =
            serde_json::from_str(&json).expect("the injected JSON parses");

        assert_eq!(value["comments"].as_array().map(Vec::len), Some(1));
        assert_eq!(value["comments"][0]["value"], " a");
    }

    #[test]
    fn test_inject_emits_full_shape_with_utf16_offsets() {
        let source: &str = "// µ";
        let mut ast: Ast = empty_ast(source);

        ast.with_mut(
            |allocator: &oxc::allocator::Allocator,
             program: &mut oxc::ast::ast::Program<'_>| {
                attach(allocator, program, vec![normalized_line(0, 5, 5)]);
            },
        );

        let spans: SpanMap = SpanMap::new(source);
        let json: String =
            inject(&ast, &spans, String::from(r#"{"type":"Program"}"#));

        assert!(json.starts_with("{\"comments\":"), "{json}");

        let value: serde_json::Value =
            serde_json::from_str(&json).expect("the injected JSON parses");
        let comment: &serde_json::Value = &value["comments"][0];

        assert_eq!(comment["type"], "Line");
        assert_eq!(comment["value"], " µ");
        assert_eq!(comment["start"], 0);
        assert_eq!(comment["end"], 4);
        assert_eq!(comment["attachedTo"], 4);
        assert_eq!(comment["position"], "leading");
        assert_eq!(comment["newlines"]["leading"], false);
        assert_eq!(comment["newlines"]["trailing"], false);
        assert_eq!(comment["content"], "none");
        assert_eq!(value["type"], "Program");
    }

    #[test]
    fn test_inject_extract_round_trip_all_comment_kinds() {
        let source: &str = concat!(
            "// line\n",
            "/* block */\n",
            "/** jsdoc */\n",
            "/*! legal */\n",
            "<!-- html open\n",
            "const x = 1;\n",
            "--> html close",
        );
        let ast: Ast = fixture_ast(source);
        let spans: SpanMap = SpanMap::new(source);
        let outbound: String = ast_to_estree_json(&ast);

        assert!(outbound.starts_with("{\"comments\":"), "{outbound}");

        let raw: Vec<RawComment> = raw_comments(&outbound);
        let (comments, _starts): (Vec<NormalizedComment>, Vec<u32>) =
            normalize_from_program(&raw, ast.program(), source, &spans)
                .expect("the outbound comments normalize");

        assert_eq!(comments.len(), 6);

        let kinds: [CommentKind; 6] = [
            CommentKind::Line,
            CommentKind::SingleLineBlock,
            CommentKind::SingleLineBlock,
            CommentKind::SingleLineBlock,
            CommentKind::HtmlOpen,
            CommentKind::HtmlClose,
        ];

        for (comment, kind) in comments.iter().zip(kinds) {
            assert_eq!(comment.kind, kind);
        }

        assert_eq!(comments[2].content, CommentContent::Jsdoc);
        assert_eq!(comments[3].content, CommentContent::Legal);

        let mut rebuilt: Ast = empty_ast(source);

        rebuilt.with_mut(
            |allocator: &oxc::allocator::Allocator,
             program: &mut oxc::ast::ast::Program<'_>| {
                attach(allocator, program, comments.clone());
            },
        );

        let second_spans: SpanMap = SpanMap::new(source);
        let second: String = inject(
            &rebuilt,
            &second_spans,
            String::from(r#"{"type":"Program"}"#),
        );

        let reextracted: Vec<NormalizedComment> = {
            let raw: Vec<RawComment> = raw_comments(&second);

            normalize_from_program(
                &raw,
                rebuilt.program(),
                source,
                &second_spans,
            )
            .expect("the reinjected comments normalize")
            .0
        };

        assert_eq!(reextracted.len(), comments.len());

        for (left, right) in comments.iter().zip(&reextracted) {
            assert_eq!(left.start, right.start);
            assert_eq!(left.end, right.end);
            assert_eq!(left.kind, right.kind);
            assert_eq!(left.content, right.content);
        }
    }

    #[test]
    fn test_extract_raw_empty_array_is_empty() {
        let comments: Vec<RawComment> = raw_comments(
            r#"{"type":"Program","comments":[],"start":0,"end":0}"#,
        );

        assert!(comments.is_empty());
    }

    #[test]
    fn test_extract_raw_without_a_comments_key_skips_parsing() {
        let comments: Vec<RawComment> = raw_comments("not json");

        assert!(comments.is_empty());
    }

    #[test]
    fn test_extract_raw_ignores_a_comments_string_value() {
        let comments: Vec<RawComment> = raw_comments(
            r#"{"type":"Program","note":"comments","body":[],"start":0,"end":0}"#,
        );

        assert!(comments.is_empty());
    }

    #[test]
    fn test_extract_raw_ignores_a_nested_comments_key() {
        let comments: Vec<RawComment> = raw_comments(concat!(
            r#"{"type":"Program","body":[{"type":"ExpressionStatement","#,
            r#""extra":{"comments":[]}}],"start":0,"end":0}"#
        ));

        assert!(comments.is_empty());
    }

    #[test]
    fn test_extract_raw_finds_a_non_first_comments_key() {
        let comments: Vec<RawComment> = raw_comments(concat!(
            r#"{"type":"Program","body":[],"start":0,"end":4,"#,
            r#""comments":[{"type":"Line","start":0,"end":4}]}"#
        ));

        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].start, Some(0));
        assert_eq!(comments[0].end, Some(4));
    }

    #[test]
    fn test_extract_raw_recognizes_an_escaped_comments_key() {
        let comments: Vec<RawComment> = raw_comments(concat!(
            r#"{"type":"Program","body":[],"start":0,"end":4,"#,
            r#""\u0063omments":[{"type":"Line","start":0,"end":4}]}"#
        ));

        assert_eq!(comments.len(), 1);
    }

    #[test]
    fn test_extract_raw_rejects_a_non_array() {
        for json in [
            r#"{"type":"Program","comments":null}"#,
            r#"{"type":"Program","comments":{}}"#,
            r#"{"type":"Program","comments":"x"}"#,
            r#"{"type":"Program","comments":5}"#,
        ] {
            let error: anyhow::Error = extract_raw(json)
                .expect_err("a non-array comments value errors");

            assert_eq!(error.to_string(), "comments must be an array");
        }
    }

    #[test]
    fn test_extract_raw_indexes_deserialization_errors() {
        let error: anyhow::Error =
            extract_raw(r#"{"type":"Program","comments":[5]}"#)
                .expect_err("the entry is not an object");

        assert!(error.to_string().starts_with("comment 0: "), "{error}");

        let error: anyhow::Error =
            extract_raw(r#"{"type":"Program","comments":[{},5]}"#)
                .expect_err("the second entry is not an object");

        assert!(error.to_string().starts_with("comment 1: "), "{error}");
    }

    #[test]
    fn test_normalize_from_program_reports_a_missing_span() {
        let source: &str = "// a";
        let spans: SpanMap = SpanMap::new(source);
        let raw: Vec<RawComment> =
            raw_comments(r#"{"type":"Program","comments":[{"type":"Line"}]}"#);
        let ast: Ast = empty_ast(source);

        let error: anyhow::Error =
            normalize_from_program(&raw, ast.program(), source, &spans)
                .expect_err("the entry has no span");

        assert_eq!(error.to_string(), "comment 0: missing start/end");
    }

    #[test]
    fn test_normalize_from_program_indexes_errors_with_offsets() {
        let source: &str = "// a";
        let spans: SpanMap = SpanMap::new(source);
        let raw: Vec<RawComment> = raw_comments(concat!(
            r#"{"type":"Program","comments":[{"type":"Line","start":0,"#,
            r#""end":4,"position":"middle"}]}"#
        ));
        let ast: Ast = empty_ast(source);

        let error: anyhow::Error =
            normalize_from_program(&raw, ast.program(), source, &spans)
                .expect_err("the position label is invalid");

        assert_eq!(
            error.to_string(),
            "comment 0 (start 0, end 4): unsupported position \"middle\""
        );
    }

    #[test]
    fn test_normalize_from_program_uses_printable_starts() {
        let source: &str = "function f(/* c */ a) { g(); }";
        let spans: SpanMap = SpanMap::new(source);
        let raw: Vec<RawComment> = raw_comments(
            r#"{"type":"Program","comments":[{"type":"Block","start":11,"end":18}]}"#,
        );
        let ast: Ast = fixture_ast(source);

        let (comments, _starts): (Vec<NormalizedComment>, Vec<u32>) =
            normalize_from_program(&raw, ast.program(), source, &spans)
                .expect("the comment normalizes");

        let statement_start: u32 =
            source.find("g();").expect("the statement is present") as u32;

        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].attached_to, statement_start);
    }

    #[test]
    fn test_normalize_from_program_rederives_stale_attached_to() {
        let source: &str = "// a\nconst x = 1;";
        let spans: SpanMap = SpanMap::new(source);
        let raw: Vec<RawComment> = raw_comments(concat!(
            r#"{"type":"Program","comments":[{"type":"Line","start":0,"#,
            r#""end":4,"attachedTo":99}]}"#
        ));
        let ast: Ast = fixture_ast(source);

        let (comments, _starts): (Vec<NormalizedComment>, Vec<u32>) =
            normalize_from_program(&raw, ast.program(), source, &spans)
                .expect("the stale anchor is re-derived");

        assert_eq!(comments[0].attached_to, 5);
    }

    #[test]
    fn test_normalize_from_program_honors_printable_attached_to() {
        let source: &str = "// a\nconst x = 1;";
        let spans: SpanMap = SpanMap::new(source);
        let raw: Vec<RawComment> = raw_comments(concat!(
            r#"{"type":"Program","comments":[{"type":"Line","start":0,"#,
            r#""end":4,"attachedTo":5}]}"#
        ));
        let ast: Ast = fixture_ast(source);

        let (comments, _starts): (Vec<NormalizedComment>, Vec<u32>) =
            normalize_from_program(&raw, ast.program(), source, &spans)
                .expect("the explicit anchor is accepted");

        assert_eq!(comments[0].attached_to, 5);
    }

    #[test]
    fn test_extract_ignores_a_supplied_value() {
        let source: &str = "// real";
        let spans: SpanMap = SpanMap::new(source);
        let raw: Vec<RawComment> = raw_comments(
            r#"{"type":"Program","sourceType":"script","body":[],"start":0,"end":7,"comments":[{"type":"Line","value":"bogus","start":0,"end":7}]}"#,
        );
        let mut rebuilt: Ast = empty_ast(source);

        let (comments, _starts): (Vec<NormalizedComment>, Vec<u32>) =
            normalize_from_program(&raw, rebuilt.program(), source, &spans)
                .expect("the comment normalizes");

        rebuilt.with_mut(
            |allocator: &oxc::allocator::Allocator,
             program: &mut oxc::ast::ast::Program<'_>| {
                attach(allocator, program, comments);
            },
        );

        let json: String =
            inject(&rebuilt, &spans, String::from(r#"{"type":"Program"}"#));

        let value: serde_json::Value =
            serde_json::from_str(&json).expect("the injected JSON parses");

        assert_eq!(value["comments"][0]["value"], " real");
    }

    #[test]
    fn test_attach_reanchors_binding_identifier_anchor() {
        let source: &str = "function f(/* c */ a) {}";
        let comment_start: u32 =
            source.find("/* c */").expect("the comment is present") as u32;
        let comment_end: u32 = comment_start + "/* c */".len() as u32;
        let binding_start: u32 =
            source.find("a)").expect("the parameter is present") as u32;
        let mut ast: Ast = rebuilt_without_comments(source);

        attach_one(
            &mut ast,
            normalized_block(comment_start, comment_end, binding_start),
        );

        let attached_to: u32 = ast.program().comments[0].attached_to;

        assert_ne!(attached_to, binding_start, "the non-printable start");
        assert_eq!(
            ast.program().comments[0].position,
            CommentPosition::Leading
        );

        let generated: String = generated(&ast);

        assert!(generated.contains("/* c */"), "{generated}");
    }

    #[test]
    fn test_reanchor_falls_back_to_program_end() {
        let source: &str = "const x = /* c */ 1;";
        let comment_start: u32 =
            source.find("/* c */").expect("the comment is present") as u32;
        let comment_end: u32 = comment_start + "/* c */".len() as u32;
        let literal_start: u32 =
            source.find("1;").expect("the initializer is present") as u32;
        let mut ast: Ast = rebuilt_without_comments(source);

        attach_one(
            &mut ast,
            normalized_block(comment_start, comment_end, literal_start),
        );

        let program_end: u32 = ast.program().span.end;

        assert_eq!(ast.program().comments[0].attached_to, program_end);

        let generated: String = generated(&ast);

        assert!(generated.contains("/* c */"), "{generated}");
    }

    #[test]
    fn test_reanchor_uses_next_statement() {
        let source: &str = "function f(/* c */ a) { g(); }";
        let comment_start: u32 =
            source.find("/* c */").expect("the comment is present") as u32;
        let comment_end: u32 = comment_start + "/* c */".len() as u32;
        let binding_start: u32 =
            source.find("a)").expect("the parameter is present") as u32;
        let statement_start: u32 =
            source.find("g();").expect("the statement is present") as u32;
        let mut ast: Ast = rebuilt_without_comments(source);

        attach_one(
            &mut ast,
            normalized_block(comment_start, comment_end, binding_start),
        );

        assert_eq!(ast.program().comments[0].attached_to, statement_start);

        let generated: String = generated(&ast);

        assert!(generated.contains("/* c */"), "{generated}");
    }

    #[test]
    fn test_reanchor_rejects_function_body_block_start() {
        let source: &str = "function f() /* c */ { g(); }";
        let comment_start: u32 =
            source.find("/* c */").expect("the comment is present") as u32;
        let comment_end: u32 = comment_start + "/* c */".len() as u32;
        let body_start: u32 =
            source.find('{').expect("the function body is present") as u32;
        let statement_start: u32 =
            source.find("g();").expect("the statement is present") as u32;
        let mut ast: Ast = rebuilt_without_comments(source);

        attach_one(
            &mut ast,
            normalized_block(comment_start, comment_end, body_start),
        );

        assert_eq!(ast.program().comments[0].attached_to, statement_start);

        let generated: String = generated(&ast);

        assert!(generated.contains("/* c */"), "{generated}");
    }

    #[test]
    fn test_reanchor_forces_leading() {
        let source: &str = "const x = 1; // trail";
        let mut ast: Ast = fixture_ast(source);

        assert_eq!(
            ast.program().comments[0].position,
            CommentPosition::Trailing
        );

        assert!(reanchor_ast(&mut ast));

        assert_eq!(
            ast.program().comments[0].position,
            CommentPosition::Leading
        );

        let generated: String = generated(&ast);

        assert!(generated.contains("// trail"), "{generated}");
    }

    #[test]
    fn test_reanchor_skips_pure_and_no_side_effects() {
        let cases: [(&str, CommentContent); 2] = [
            ("const x = /*#__PURE__*/ f();", CommentContent::Pure),
            (
                "const x = /*#__NO_SIDE_EFFECTS__*/ function f() {};",
                CommentContent::NoSideEffects,
            ),
        ];

        for (source, content) in cases {
            let mut ast: Ast = fixture_ast(source);
            let comment: Comment = ast.program().comments[0].clone();

            assert_eq!(comment.content, content, "{source}");

            assert!(
                !reanchor_ast(&mut ast),
                "{source} must not be re-anchored"
            );

            assert_eq!(
                ast.program().comments[0].attached_to,
                comment.attached_to
            );
            assert_eq!(ast.program().comments[0].position, comment.position);
            assert_eq!(ast.program().comments[0].content, comment.content);
        }
    }
}
