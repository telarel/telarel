use oxc::ast::ast::{CommentContent, CommentKind};

fn is_block(kind: CommentKind) -> bool {
    matches!(kind, CommentKind::SingleLineBlock | CommentKind::MultiLineBlock)
}

fn is_property_key_annotation(source: &str) -> bool {
    matches!(source.trim().strip_prefix(['@', '#']), Some("__KEY__"))
}

fn contains_license_or_preserve(value: &str) -> bool {
    value.contains("@license") || value.contains("@preserve")
}

fn is_coverage_ignore_file(source: &[u8]) -> bool {
    fn starts_with_directive(
        source: &[u8],
        directive: &[u8],
    ) -> bool {
        source.strip_prefix(directive).is_some_and(|rest: &[u8]| {
            rest.first().is_none_or(u8::is_ascii_whitespace)
        })
    }

    starts_with_directive(source, b"v8 ignore file")
        || starts_with_directive(source, b"istanbul ignore file")
}

/// Derive a missing content label for a comment from its delimiter-free text.
///
/// This mirrors oxc's text-only annotation rules. Parser context is
/// unavailable here, so `@__PURE__` and `@__NO_SIDE_EFFECTS__` classify as
/// the `*-not-applied` variants rather than their applied counterparts.
pub fn classify_content(
    value: &str,
    kind: CommentKind,
) -> CommentContent {
    let bytes: &[u8] = value.as_bytes();

    if bytes.is_empty() {
        return CommentContent::None;
    }

    match bytes[0] {
        | b'!' => return CommentContent::Legal,
        | b'*' if is_block(kind) => {
            if !bytes.iter().all(|&byte: &u8| byte == b'*') {
                return if contains_license_or_preserve(value) {
                    CommentContent::JsdocLegal
                } else {
                    CommentContent::Jsdoc
                };
            }

            return CommentContent::None;
        },
        | _ => {},
    }

    let mut start: usize = 0;

    while start < bytes.len() && bytes[start].is_ascii_whitespace() {
        start += 1;
    }

    if start >= bytes.len() {
        return CommentContent::None;
    }

    let rest: &[u8] = &bytes[start..];

    if (rest.starts_with(b"@__KEY__")
        || rest.starts_with(b"#__KEY__")
        || !rest[0].is_ascii())
        && is_property_key_annotation(value)
    {
        return CommentContent::PropertyKey;
    }

    match bytes[start] {
        | b'@' => {
            start += 1;

            if start >= bytes.len() {
                return CommentContent::None;
            }

            if bytes[start..].starts_with(b"vite") {
                return CommentContent::Vite;
            }

            if bytes[start..].starts_with(b"license")
                || bytes[start..].starts_with(b"preserve")
            {
                return CommentContent::Legal;
            }
        },
        | b'#' => {
            start += 1;
        },
        | b'w' => {
            if bytes[start..].starts_with(b"webpack")
                && start + 7 < bytes.len()
                && bytes[start + 7].is_ascii_uppercase()
            {
                return CommentContent::Webpack;
            }
        },
        | b't' => {
            if bytes[start..].starts_with(b"turbopack")
                && start + 9 < bytes.len()
                && bytes[start + 9].is_ascii_uppercase()
            {
                return CommentContent::Turbopack;
            }
        },
        | b'v' | b'c' | b'n' | b'i' => {
            let rest: &[u8] = &bytes[start..];

            if rest.starts_with(b"v8 ignore")
                || rest.starts_with(b"c8 ignore")
                || rest.starts_with(b"node:coverage")
                || rest.starts_with(b"istanbul ignore")
            {
                return if is_coverage_ignore_file(rest) {
                    CommentContent::CoverageIgnoreFile
                } else {
                    CommentContent::CoverageIgnore
                };
            }
        },
        | _ => {
            return if contains_license_or_preserve(value) {
                CommentContent::Legal
            } else {
                CommentContent::None
            };
        },
    }

    if start < bytes.len() && bytes[start..].starts_with(b"__") {
        let rest: &[u8] = &bytes[start + 2..];

        if rest.starts_with(b"PURE__") {
            return CommentContent::PureNotApplied;
        }

        if rest.starts_with(b"NO_SIDE_EFFECTS__") {
            return CommentContent::NoSideEffectsNotApplied;
        }
    }

    if contains_license_or_preserve(value) {
        return CommentContent::Legal;
    }

    CommentContent::None
}

#[cfg(test)]
mod tests {
    use oxc::ast::ast::{CommentContent, CommentKind};

    use crate::comment::classify::classify_content;

    fn classify_block(value: &str) -> CommentContent {
        classify_content(value, CommentKind::SingleLineBlock)
    }

    #[test]
    fn test_classify_legal_bang() {
        assert_eq!(classify_block("! license"), CommentContent::Legal);
        assert_eq!(
            classify_content("! license", CommentKind::Line),
            CommentContent::Legal
        );
    }

    #[test]
    fn test_classify_jsdoc_variants() {
        assert_eq!(classify_block("* docs "), CommentContent::Jsdoc);

        assert_eq!(
            classify_block("* @license MIT "),
            CommentContent::JsdocLegal
        );

        assert_eq!(classify_block("* @preserve "), CommentContent::JsdocLegal);

        assert_eq!(classify_block("***"), CommentContent::None);
    }

    #[test]
    fn test_classify_property_key() {
        assert_eq!(classify_block(" @__KEY__ "), CommentContent::PropertyKey);

        assert_eq!(classify_block("#__KEY__"), CommentContent::PropertyKey);

        assert_eq!(
            classify_block("\u{00a0}@__KEY__"),
            CommentContent::PropertyKey
        );
    }

    #[test]
    fn test_classify_annotations() {
        let cases: [(&str, CommentContent); 12] = [
            ("@vite", CommentContent::Vite),
            ("@license MIT", CommentContent::Legal),
            ("@preserve", CommentContent::Legal),
            ("webpackChunkName", CommentContent::Webpack),
            ("turbopackModule", CommentContent::Turbopack),
            ("v8 ignore next", CommentContent::CoverageIgnore),
            ("v8 ignore file", CommentContent::CoverageIgnoreFile),
            ("c8 ignore next", CommentContent::CoverageIgnore),
            ("node:coverage disable", CommentContent::CoverageIgnore),
            ("istanbul ignore next", CommentContent::CoverageIgnore),
            ("istanbul ignore file", CommentContent::CoverageIgnoreFile),
            ("@__PURE__", CommentContent::PureNotApplied),
        ];

        for (value, expected) in cases {
            assert_eq!(classify_block(value), expected, "{value}");
        }

        assert_eq!(classify_block("#__PURE__"), CommentContent::PureNotApplied);

        assert_eq!(
            classify_block("@__NO_SIDE_EFFECTS__"),
            CommentContent::NoSideEffectsNotApplied
        );

        assert_eq!(
            classify_block("#__NO_SIDE_EFFECTS__"),
            CommentContent::NoSideEffectsNotApplied
        );
    }

    #[test]
    fn test_classify_license_anywhere_and_plain() {
        assert_eq!(
            classify_block("some text @license here"),
            CommentContent::Legal
        );

        assert_eq!(classify_block("plain text"), CommentContent::None);

        assert_eq!(classify_block(""), CommentContent::None);
    }

    #[test]
    fn test_classify_webpack_requires_uppercase() {
        assert_eq!(classify_block("webpack"), CommentContent::None);
        assert_eq!(classify_block("webpackchunk"), CommentContent::None);
    }
}
