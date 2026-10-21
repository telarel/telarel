use std::path::Path;

/// Render a Rust path as the string label the JS boundary exposes.
///
/// Rust paths may be non-UTF-8 on Unix while JS strings cannot represent them,
/// so the conversion is lossy by design: non-UTF-8 bytes become `U+FFFD`.
/// Internal resolution stays on `Path`.
pub fn path_label(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_label_renders_a_utf8_path() {
        assert_eq!(path_label(Path::new("/repo")), "/repo");
    }

    #[test]
    fn test_path_label_renders_the_empty_path_as_empty_string() {
        assert_eq!(path_label(Path::new("")), "");
    }

    #[cfg(unix)]
    #[test]
    fn test_path_label_replaces_non_utf8_bytes() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;

        let path: &Path = Path::new(OsStr::from_bytes(b"cwd-\xFF"));

        assert_eq!(path_label(path), "cwd-\u{FFFD}");
    }
}
