fn skip_whitespace(
    bytes: &[u8],
    mut index: usize,
) -> usize {
    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }

    index
}

/// Skip a JSON string literal. `index` points at the opening quote.
fn skip_string(
    bytes: &[u8],
    index: usize,
) -> Option<usize> {
    let mut index: usize = index + 1;

    while index < bytes.len() {
        match bytes[index] {
            | b'\\' => index += 2,
            | b'"' => return Some(index + 1),
            | _ => index += 1,
        }
    }

    None
}

/// Skip one JSON value and return the index just after it.
fn skip_value(
    bytes: &[u8],
    index: usize,
) -> Option<usize> {
    match *bytes.get(index)? {
        | b'"' => skip_string(bytes, index),
        | b'{' | b'[' => {
            let mut depth: usize = 0;

            let mut index: usize = index;

            while index < bytes.len() {
                match bytes[index] {
                    | b'"' => index = skip_string(bytes, index)?,
                    | b'{' | b'[' => {
                        depth += 1;
                        index += 1;
                    },
                    | b'}' | b']' => {
                        depth -= 1;
                        index += 1;

                        if depth == 0 {
                            return Some(index);
                        }
                    },
                    | _ => index += 1,
                }
            }

            None
        },
        | _ => {
            let start: usize = index;

            let mut index: usize = index;

            while index < bytes.len()
                && !matches!(
                    bytes[index],
                    b',' | b'}' | b']' | b' ' | b'\t' | b'\r' | b'\n'
                )
            {
                index += 1;
            }

            (index > start).then_some(index)
        },
    }
}

fn is_comments_key(literal: &str) -> bool {
    if literal == "\"comments\"" {
        return true;
    }

    if !literal.contains('\\') {
        return false;
    }

    serde_json::from_str::<String>(literal)
        .is_ok_and(|key: String| key == "comments")
}

/// Return the JSON array slice for the top-level `comments` key.
///
/// The function returns `None` when the input is not an object or has no
/// top-level `comments` key. A present non-array value errors with the public
/// message `comments must be an array`.
pub fn comments_value(program_json: &str) -> anyhow::Result<Option<&str>> {
    let bytes: &[u8] = program_json.as_bytes();

    let mut index: usize = skip_whitespace(bytes, 0);

    if bytes.get(index) != Some(&b'{') {
        return Ok(None);
    }

    index += 1;

    loop {
        index = skip_whitespace(bytes, index);

        match bytes.get(index) {
            | Some(b'"') => {},
            | _ => return Ok(None),
        }

        let literal_start: usize = index;

        let Some(literal_end): Option<usize> = skip_string(bytes, index) else {
            return Ok(None);
        };

        index = skip_whitespace(bytes, literal_end);

        if bytes.get(index) != Some(&b':') {
            return Ok(None);
        }

        index = skip_whitespace(bytes, index + 1);

        if is_comments_key(&program_json[literal_start..literal_end]) {
            if bytes.get(index) != Some(&b'[') {
                anyhow::bail!("comments must be an array");
            }

            let Some(end): Option<usize> = skip_value(bytes, index) else {
                return Ok(None);
            };

            return Ok(Some(&program_json[index..end]));
        }

        let Some(end): Option<usize> = skip_value(bytes, index) else {
            return Ok(None);
        };

        index = skip_whitespace(bytes, end);

        match bytes.get(index) {
            | Some(b',') => index += 1,
            | _ => return Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::comment::scan::comments_value;

    #[test]
    fn test_comments_value_finds_a_non_first_key() {
        let json: &str = concat!(
            r#"{"type":"Program","body":[],"start":0,"end":0,"#,
            r#""comments":[{"type":"Line","start":0,"end":4}]}"#
        );

        assert_eq!(
            comments_value(json).expect("the scan succeeds"),
            Some(r#"[{"type":"Line","start":0,"end":4}]"#)
        );
    }

    #[test]
    fn test_comments_value_returns_none_without_a_key() {
        let json: &str = r#"{"type":"Program","body":[],"start":0,"end":0}"#;

        assert_eq!(comments_value(json).expect("the scan succeeds"), None);
    }

    #[test]
    fn test_comments_value_returns_none_for_a_non_object() {
        assert_eq!(
            comments_value("not json").expect("the scan succeeds"),
            None
        );
        assert_eq!(comments_value("[]").expect("the scan succeeds"), None);
    }

    #[test]
    fn test_comments_value_skips_escaped_quotes_in_values() {
        let json: &str = r#"{"type":"Program","note":"a\"comments\"b"}"#;

        assert_eq!(comments_value(json).expect("the scan succeeds"), None);
    }

    #[test]
    fn test_comments_value_skips_unicode_escapes_in_values() {
        let json: &str =
            r#"{"type":"Program","note":"a\u0022comments\u0022b"}"#;

        assert_eq!(comments_value(json).expect("the scan succeeds"), None);
    }

    #[test]
    fn test_comments_value_skips_escaped_quotes_and_backslashes_in_keys() {
        let json: &str = concat!(
            r#"{"key\"quote\\backslash":"value","#,
            r#""comments":[{"type":"Line","start":0,"end":4}]}"#
        );

        assert_eq!(
            comments_value(json).expect("the scan succeeds"),
            Some(r#"[{"type":"Line","start":0,"end":4}]"#)
        );
    }

    #[test]
    fn test_comments_value_recognizes_an_escaped_key() {
        let json: &str = r#"{"type":"Program","\u0063omments":[]}"#;

        assert_eq!(
            comments_value(json).expect("the scan succeeds"),
            Some("[]")
        );
    }

    #[test]
    fn test_comments_value_ignores_a_nested_comments_key() {
        let json: &str = concat!(
            r#"{"type":"Program","body":[{"type":"ExpressionStatement","#,
            r#""extra":{"comments":[]}}],"start":0,"end":0}"#
        );

        assert_eq!(comments_value(json).expect("the scan succeeds"), None);
    }

    #[test]
    fn test_comments_value_handles_brackets_inside_strings() {
        let json: &str = r#"{"type":"Program","comments":[{"value":"]"}]}"#;

        assert_eq!(
            comments_value(json).expect("the scan succeeds"),
            Some(r#"[{"value":"]"}]"#)
        );
    }

    #[test]
    fn test_comments_value_skips_nested_objects_and_arrays() {
        let json: &str = concat!(
            r#"{"type":"Program","meta":{"a":[1,{"b":"}"}]},"#,
            r#""comments":[]}"#
        );

        assert_eq!(
            comments_value(json).expect("the scan succeeds"),
            Some("[]")
        );
    }

    #[test]
    fn test_comments_value_rejects_a_non_array() {
        for json in [
            r#"{"type":"Program","comments":null}"#,
            r#"{"type":"Program","comments":{}}"#,
            r#"{"type":"Program","comments":"x"}"#,
            r#"{"type":"Program","comments":5}"#,
        ] {
            let error: anyhow::Error = comments_value(json)
                .expect_err("a non-array comments value errors");

            assert_eq!(error.to_string(), "comments must be an array");
        }
    }
}
