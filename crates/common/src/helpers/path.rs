use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// The process working directory, or `/` when it cannot be read.
pub fn get_process_cwd() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"))
}

/// Resolve `path` against `cwd` unless `path` is already absolute.
pub fn resolve_against(
    cwd: &Path,
    path: &Path,
) -> PathBuf {
    if path.is_absolute() { path.to_path_buf() } else { cwd.join(path) }
}

/// Drop the `?query`/`#hash` suffix from a file.
pub fn strip_file_suffix(file: &str) -> &str {
    let without_hash: &str =
        file.split_once('#').map_or(file, |(path, _)| path);

    without_hash.split_once('?').map_or(without_hash, |(path, _)| path)
}

/// The external source-map path for `file`: the suffix-stripped file plus
/// `.map`, resolved against `cwd` unless absolute.
pub fn external_map_path(
    cwd: &Path,
    file: &str,
) -> PathBuf {
    let file: &str = strip_file_suffix(file);

    let mut path: OsString =
        resolve_against(cwd, Path::new(file)).into_os_string();

    path.push(".map");

    PathBuf::from(path)
}

/// The source-map URL derived from `file`: the suffix-stripped file plus
/// `.map`, written as a URL path.
///
/// Returns `None` when `file` is empty after stripping its suffix.
pub fn external_map_url(file: &str) -> Option<String> {
    let file: &str = strip_file_suffix(file);

    if file.is_empty() {
        return None;
    }

    Some(format!("{file}.map").replace('\\', "/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_against_absolute_path_wins() {
        let absolute: PathBuf = std::env::current_dir()
            .expect("process cwd readable")
            .join("abs.ts");

        let path: PathBuf = resolve_against(Path::new("/repo"), &absolute);

        assert_eq!(path, absolute);
    }

    #[test]
    fn test_resolve_against_relative_path_joins_cwd() {
        let path: PathBuf =
            resolve_against(Path::new("/repo"), Path::new("src/index.ts"));

        assert_eq!(path, PathBuf::from("/repo/src/index.ts"));
    }

    #[test]
    fn test_strip_file_suffix_removes_query() {
        assert_eq!(strip_file_suffix("index.ts?v=1"), "index.ts");
    }

    #[test]
    fn test_strip_file_suffix_removes_hash() {
        assert_eq!(strip_file_suffix("index.ts#hash"), "index.ts");
    }

    #[test]
    fn test_strip_file_suffix_removes_query_and_hash() {
        assert_eq!(strip_file_suffix("entry.ts?v=1#h"), "entry.ts");
    }

    #[test]
    fn test_external_map_path_resolves_relative_file() {
        let path: PathBuf =
            external_map_path(Path::new("/repo"), "src/index.ts");

        assert_eq!(path, PathBuf::from("/repo/src/index.ts.map"));
    }

    #[test]
    fn test_external_map_url_appends_map() {
        assert_eq!(
            external_map_url("dist/index.js"),
            Some(String::from("dist/index.js.map")),
        );
    }

    #[test]
    fn test_external_map_url_strips_file_suffix() {
        assert_eq!(
            external_map_url("dist/index.js?v=1#h"),
            Some(String::from("dist/index.js.map")),
        );
    }

    #[test]
    fn test_external_map_url_empty_file_is_none() {
        assert_eq!(external_map_url(""), None);
        assert_eq!(external_map_url("?v=1"), None);
        assert_eq!(external_map_url("#h"), None);
    }

    #[test]
    fn test_external_map_url_normalizes_backslashes() {
        assert_eq!(
            external_map_url(r"dist\index.js"),
            Some(String::from("dist/index.js.map")),
        );
    }

    #[test]
    fn test_external_map_path_dot_prefix_matches_plain_relative() {
        let path: PathBuf =
            external_map_path(Path::new("/repo"), "./src/index.ts");

        assert_eq!(path, PathBuf::from("/repo/src/index.ts.map"));
    }

    #[test]
    fn test_external_map_path_absolute_file_ignores_cwd() {
        let path: PathBuf =
            external_map_path(Path::new("/repo"), "/abs/index.ts");

        assert_eq!(path, PathBuf::from("/abs/index.ts.map"));
    }

    #[test]
    fn test_external_map_path_appends_map_without_replacing_extension() {
        let path: PathBuf = external_map_path(Path::new("/repo"), "index.ts");

        assert_eq!(path, PathBuf::from("/repo/index.ts.map"));
        assert_ne!(path, PathBuf::from("/repo/index.map"));
    }

    #[test]
    fn test_external_map_path_strips_file_suffix() {
        let path: PathBuf =
            external_map_path(Path::new("/repo"), "entry.ts?v=1#h");

        assert_eq!(path, PathBuf::from("/repo/entry.ts.map"));
    }

    #[test]
    fn test_external_map_path_trailing_cwd_separator_is_equivalent() {
        let with_separator: PathBuf =
            external_map_path(Path::new("/repo/"), "src/index.ts");
        let without_separator: PathBuf =
            external_map_path(Path::new("/repo"), "src/index.ts");

        assert_eq!(with_separator, without_separator);
    }

    #[cfg(windows)]
    #[test]
    fn test_external_map_path_windows_separators() {
        let path: PathBuf =
            external_map_path(Path::new(r"C:\repo"), r"src\index.ts");

        assert_eq!(path, PathBuf::from(r"C:\repo\src\index.ts.map"));
    }
}
