/// A boundary table mapping UTF-8 byte offsets to UTF-16 code-unit offsets
/// for one source text, so the Rust `Span` type keeps its byte semantics
/// across the JS bridge.
pub struct SpanMap {
    boundaries: Vec<(usize, usize)>,
}

impl SpanMap {
    pub fn new(source: &str) -> Self {
        let mut boundaries: Vec<(usize, usize)> =
            Vec::with_capacity(source.len() + 1);

        let mut utf16_len: usize = 0;

        boundaries.push((0, 0));

        for (byte_index, character) in source.char_indices() {
            utf16_len += character.len_utf16();
            boundaries.push((byte_index + character.len_utf8(), utf16_len));
        }

        Self { boundaries }
    }

    /// Out-of-range offsets clamp to the nearest boundary and never panic.
    pub fn byte_to_utf16(
        &self,
        byte_offset: usize,
    ) -> usize {
        let index: usize =
            self.boundaries.partition_point(|(byte, _)| *byte <= byte_offset);

        self.boundaries[index.saturating_sub(1).min(self.boundaries.len() - 1)]
            .1
    }

    /// Out-of-range offsets clamp to the nearest boundary and never panic.
    pub fn utf16_to_byte(
        &self,
        utf16_index: usize,
    ) -> usize {
        let index: usize =
            self.boundaries.partition_point(|(_, utf16)| *utf16 <= utf16_index);

        self.boundaries[index.saturating_sub(1).min(self.boundaries.len() - 1)]
            .0
    }
}

/// Rewrite the integer value following every `start` or `end` key in a
/// serialized JSON string, mapping each through `convert`. Values that happen
/// to read `"start"`/`"end"` are left alone because only a key is recognized.
fn rewrite_spans(
    json: &str,
    convert: impl Fn(usize) -> usize,
) -> String {
    let bytes: &[u8] = json.as_bytes();
    let mut output: String = String::with_capacity(json.len());
    let mut index: usize = 0;

    while index < bytes.len() {
        if bytes[index] != b'"' {
            // Copy bytes up to the next quote. A UTF-8 continuation byte is
            // always >= 0x80, so 0x22 can only be a real quote: the run ends
            // on a char boundary and the slice stays valid UTF-8.
            let run_start: usize = index;

            while index < bytes.len() && bytes[index] != b'"' {
                index += 1;
            }

            output.push_str(&json[run_start..index]);

            continue;
        }

        let literal_start: usize = index;

        index += 1;

        while index < bytes.len() {
            match bytes[index] {
                | b'\\' => index = (index + 2).min(bytes.len()),
                | b'"' => {
                    index += 1;
                    break;
                },
                | _ => index += 1,
            }
        }

        let literal: &str = &json[literal_start..index];

        output.push_str(literal);

        let mut separator: usize = index;

        while separator < bytes.len() && bytes[separator].is_ascii_whitespace()
        {
            separator += 1;
        }

        let is_span_key: bool = literal == "\"start\"" || literal == "\"end\"";

        if is_span_key && bytes.get(separator) == Some(&b':') {
            output.push_str(&json[index..=separator]);

            index = separator + 1;

            while index < bytes.len() && bytes[index].is_ascii_whitespace() {
                output.push(bytes[index] as char);
                index += 1;
            }

            let value_start: usize = index;

            while index < bytes.len() && bytes[index].is_ascii_digit() {
                index += 1;
            }

            if index > value_start {
                let value: usize =
                    json[value_start..index].parse().unwrap_or_default();
                output.push_str(&convert(value).to_string());
            }
        }
    }

    output
}

/// Convert every `start`/`end` span from byte offsets to UTF-16 code units.
pub fn spans_to_utf16(
    map: &SpanMap,
    json: &str,
) -> String {
    rewrite_spans(json, |byte: usize| map.byte_to_utf16(byte))
}

/// Convert every `start`/`end` span from UTF-16 code units back to byte offsets.
pub fn spans_to_bytes(
    map: &SpanMap,
    json: &str,
) -> String {
    rewrite_spans(json, |utf16: usize| map.utf16_to_byte(utf16))
}

#[cfg(test)]
mod tests {
    use super::{SpanMap, spans_to_bytes, spans_to_utf16};

    #[test]
    fn test_byte_to_utf16_ascii_is_identity() {
        let source: &str = "const a = 1;";
        let map: SpanMap = SpanMap::new(source);

        for offset in 0..=source.len() {
            assert_eq!(map.byte_to_utf16(offset), offset);
            assert_eq!(map.utf16_to_byte(offset), offset);
        }
    }

    #[test]
    fn test_byte_to_utf16_micro_is_two_bytes_one_unit() {
        let source: &str = "const µ = 1;";
        let map: SpanMap = SpanMap::new(source);

        assert_eq!(map.byte_to_utf16(0), 0);
        assert_eq!(map.byte_to_utf16(6), 6);
        assert_eq!(map.byte_to_utf16(8), 7);
        assert_eq!(map.byte_to_utf16(source.len()), 12);
    }

    #[test]
    fn test_byte_to_utf16_cjk_is_three_bytes_one_unit() {
        let source: &str = "日";
        let map: SpanMap = SpanMap::new(source);

        assert_eq!(map.byte_to_utf16(0), 0);
        assert_eq!(map.byte_to_utf16(source.len()), 1);
        assert_eq!(map.utf16_to_byte(1), 3);
    }

    #[test]
    fn test_byte_to_utf16_emoji_is_four_bytes_two_units() {
        let source: &str = "😀";
        let map: SpanMap = SpanMap::new(source);

        assert_eq!(map.byte_to_utf16(0), 0);
        assert_eq!(map.byte_to_utf16(source.len()), 2);
        assert_eq!(map.utf16_to_byte(2), 4);
    }

    #[test]
    fn test_byte_to_utf16_mixed_round_trip_at_every_boundary() {
        let source: &str = "a日😀b";
        let map: SpanMap = SpanMap::new(source);
        let byte_boundaries: [usize; 5] = [0, 1, 4, 8, 9];

        for boundary in byte_boundaries {
            let utf16: usize = map.byte_to_utf16(boundary);

            assert_eq!(
                map.utf16_to_byte(utf16),
                boundary,
                "round trip at byte {boundary}"
            );
        }
    }

    #[test]
    fn test_byte_to_utf16_clamps_out_of_range() {
        let source: &str = "a日😀b";
        let map: SpanMap = SpanMap::new(source);

        assert_eq!(map.byte_to_utf16(usize::MAX), 5);
        assert_eq!(map.utf16_to_byte(usize::MAX), 9);
    }

    #[test]
    fn test_spans_to_utf16_rewrites_only_span_keys() {
        let source: &str = "日a";
        let map: SpanMap = SpanMap::new(source);
        let json: &str = concat!(
            r#"{"type":"Foo","name":"start","start":3,"end":4,"#,
            r#""range":[3,4]}"#
        );

        let converted: String = spans_to_utf16(&map, json);

        assert_eq!(
            converted,
            concat!(
                r#"{"type":"Foo","name":"start","start":1,"end":2,"#,
                r#""range":[3,4]}"#
            )
        );
    }

    #[test]
    fn test_spans_to_utf16_copies_escapes_verbatim() {
        let source: &str = "日a";
        let map: SpanMap = SpanMap::new(source);
        let json: &str = r#"{"name":"a\"start\"","start":3,"end":4}"#;

        let converted: String = spans_to_utf16(&map, json);

        assert_eq!(converted, r#"{"name":"a\"start\"","start":1,"end":2}"#);
    }

    #[test]
    fn test_spans_to_bytes_round_trips_span_keys() {
        let source: &str = "日a";
        let map: SpanMap = SpanMap::new(source);
        let json: &str = r#"{"start":3,"end":4}"#;

        let utf16: String = spans_to_utf16(&map, json);
        let bytes: String = spans_to_bytes(&map, &utf16);

        assert_eq!(utf16, r#"{"start":1,"end":2}"#);
        assert_eq!(bytes, json);
    }
}
