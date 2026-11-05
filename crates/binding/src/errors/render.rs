use oxc::span::Span;
use telarel_common::CompileError;

use crate::plugin::spans::SpanMap;

/// Clamp a JavaScript number to a non-negative integer offset.
fn utf16_offset(value: f64) -> usize {
    if !value.is_finite() || value <= 0.0 {
        return 0;
    }

    if value >= usize::MAX as f64 {
        return usize::MAX;
    }

    value.trunc() as usize
}

/// Render a code frame with the compiler's native renderer.
///
/// `start` and `end` are UTF-16 code-unit offsets. The function clamps them
/// to the source bounds and returns the frame without its outer newlines.
#[napi_derive::napi]
pub fn render_code_frame(
    file: String,
    code: String,
    start: f64,
    end: f64,
    message: String,
) -> String {
    let mut start: usize = utf16_offset(start);

    let mut end: usize = utf16_offset(end);

    if start > end {
        std::mem::swap(&mut start, &mut end);
    }

    let map: SpanMap = SpanMap::new(&code);

    let start: u32 =
        u32::try_from(map.utf16_to_byte(start)).unwrap_or(u32::MAX);

    let end: u32 = u32::try_from(map.utf16_to_byte(end)).unwrap_or(u32::MAX);

    CompileError::new(message)
        .with_source_code(file, code, Span::new(start, end))
        .to_string()
        .trim_matches('\n')
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_renders_the_native_frame() {
        let rendered: String = render_code_frame(
            "src/app.js".to_owned(),
            "let 1 = x;".to_owned(),
            4.0,
            5.0,
            "expected an identifier".to_owned(),
        );

        assert!(rendered.contains("src/app.js"));
        assert!(rendered.contains("expected an identifier"));
        assert!(rendered.contains(",-["));
        assert!(rendered.contains("^"));
        assert!(rendered.contains("`----"));
        assert!(!rendered.starts_with('\n'));
        assert!(!rendered.ends_with('\n'));
    }

    #[test]
    fn test_clamps_out_of_range_offsets() {
        let rendered: String = render_code_frame(
            "input.ts".to_owned(),
            "abc".to_owned(),
            99.0,
            100.0,
            "boom".to_owned(),
        );

        assert!(rendered.contains("input.ts"));
        assert!(rendered.contains("boom"));
        assert!(rendered.contains("abc"));
        assert!(!rendered.contains("NaN"));
    }

    #[test]
    fn test_clamps_non_finite_offsets() {
        let rendered: String = render_code_frame(
            "input.ts".to_owned(),
            "abc".to_owned(),
            f64::NAN,
            f64::INFINITY,
            "boom".to_owned(),
        );

        assert!(rendered.contains("input.ts"));
        assert!(rendered.contains("boom"));
        assert!(rendered.contains("abc"));
    }

    #[test]
    fn test_swaps_reversed_offsets() {
        let rendered: String = render_code_frame(
            "input.ts".to_owned(),
            "const x = 1;".to_owned(),
            7.0,
            6.0,
            "boom".to_owned(),
        );

        assert!(rendered.contains("input.ts"));
        assert!(rendered.contains("^"));
    }

    #[test]
    fn test_converts_utf16_offsets() {
        let rendered: String = render_code_frame(
            "input.ts".to_owned(),
            "😀日x".to_owned(),
            2.0,
            3.0,
            "boom".to_owned(),
        );

        assert!(rendered.contains("input.ts"));
        assert!(rendered.contains("😀日x"));
        assert!(rendered.contains("^"));
    }

    #[test]
    fn test_renders_an_empty_source() {
        let rendered: String = render_code_frame(
            "input.ts".to_owned(),
            String::new(),
            0.0,
            0.0,
            "boom".to_owned(),
        );

        assert!(rendered.contains("input.ts"));
        assert!(rendered.contains("boom"));
    }
}
