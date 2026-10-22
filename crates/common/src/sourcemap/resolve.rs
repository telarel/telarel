use std::path::{Path, PathBuf};

use crate::_types::options::sourcemap::input::{
    InputSourceMap, InputSourceMapMode,
};
use crate::errors::compile::CompileError;
use crate::helpers::path::{
    external_map_path, resolve_against, strip_file_suffix,
};
use crate::sourcemap::SourceMap;
use crate::sourcemap::inline::extract_inline_source_map;

/// Parse a source map JSON string, naming its source in any error.
fn parse_map_json(
    json: &str,
    source: &str,
) -> Result<SourceMap, CompileError> {
    let map: oxc_sourcemap::SourceMap<'_> =
        oxc_sourcemap::SourceMap::from_json_string(json).map_err(
            |error: oxc_sourcemap::Error| {
                CompileError::new(format!(
                    "invalid source map `{source}`: {error}",
                ))
            },
        )?;

    Ok(map.into_owned())
}

/// Read and parse a source map file, naming it in any error.
fn read_map_file(path: &Path) -> Result<SourceMap, CompileError> {
    let json: String =
        std::fs::read_to_string(path).map_err(|error: std::io::Error| {
            CompileError::new(format!(
                "cannot read source map `{}`: {error}",
                path.display(),
            ))
        })?;

    parse_map_json(&json, &path.display().to_string())
}

/// The directory of `file`, used to resolve a relative external map `path`.
///
/// An empty suffix-stripped `file` falls back to `cwd`.
fn source_dir(
    cwd: &Path,
    file: &str,
) -> PathBuf {
    let file: &str = strip_file_suffix(file);

    if file.is_empty() {
        return cwd.to_path_buf();
    }

    let resolved: PathBuf = resolve_against(cwd, Path::new(file));

    resolved
        .parent()
        .map_or_else(|| cwd.to_path_buf(), |parent: &Path| parent.to_path_buf())
}

/// The external map path for `file`, or the configured `path` override.
fn external_map_path_for(
    cwd: &Path,
    file: &str,
    path: Option<&Path>,
) -> PathBuf {
    match path {
        | None => external_map_path(cwd, file),
        | Some(path) if path.is_absolute() => path.to_path_buf(),
        | Some(path) => source_dir(cwd, file).join(path),
    }
}

/// Resolve the map for a concrete [`InputSourceMapMode`].
fn resolve_input_source_map_mode(
    code: &str,
    cwd: &Path,
    file: &str,
    mode: &InputSourceMapMode,
    path: Option<&Path>,
) -> Result<Option<SourceMap>, CompileError> {
    match mode {
        | InputSourceMapMode::Value(map) => Ok(Some(map.clone())),
        | InputSourceMapMode::Inline => extract_inline_source_map(code)
            .map_or(Ok(None), |result| result.map(Some)),
        | InputSourceMapMode::External => {
            let map_path: PathBuf = external_map_path_for(cwd, file, path);
            let map: SourceMap = read_map_file(&map_path)?;

            Ok(Some(map))
        },
        | InputSourceMapMode::Auto => match extract_inline_source_map(code) {
            | Some(Ok(map)) => Ok(Some(map)),
            | Some(Err(error)) => Err(error),
            | None => {
                let map_path: PathBuf = external_map_path_for(cwd, file, path);

                if map_path.is_file() {
                    let map: SourceMap = read_map_file(&map_path)?;

                    Ok(Some(map))
                } else {
                    Ok(None)
                }
            },
        },
    }
}

/// Resolve the input source map requested by `input`.
///
/// `code` is the source being parsed (`Inline`/`Auto` scan it for an inline
/// comment). `cwd` is the root used to locate the source file. `External` and
/// the `Auto` fallback read the configured `path` when set, or `<file>.map`
/// otherwise; a relative `path` resolves against the source file's directory,
/// and the default path strips any `?query`/`#hash` suffix.
///
/// Returns `Ok(None)` only when the mode is best-effort (`Inline`/`Auto`) and
/// no map was found. Explicitly requested but missing maps (`External`) and
/// corrupt data are `Err`.
pub fn resolve_input_source_map(
    code: &str,
    cwd: &Path,
    file: &str,
    input: &InputSourceMap,
) -> Result<Option<SourceMap>, CompileError> {
    let (mode, path): (&InputSourceMapMode, Option<&Path>) = match input {
        | InputSourceMap::Mode(mode) => (mode, None),
        | InputSourceMap::Options(options) => {
            (&options.mode, options.path.as_deref())
        },
    };

    resolve_input_source_map_mode(code, cwd, file, mode, path)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use oxc_sourcemap::SourceMapBuilder;
    use tempfile::TempDir;

    use crate::_types::options::sourcemap::input::InputSourceMapOptions;
    use crate::helpers::test_util::{path_buf, temp_dir, write_file};
    use crate::sourcemap::inline::append_inline_source_map;

    use super::*;

    fn sample_map(file: &str) -> SourceMap {
        let mut builder: SourceMapBuilder<'_> = SourceMapBuilder::default();

        builder.set_file(file);

        let source_id: u32 =
            builder.add_source_and_content(file, "const a = 1;");

        builder.add_token(0, 0, 4, 0, Some(source_id), None);

        builder.into_owned_sourcemap().into_inner()
    }

    fn json_of(map: &SourceMap) -> String {
        map.to_json_string()
    }

    fn assert_same_map(
        actual: &SourceMap,
        expected: &SourceMap,
    ) {
        assert_eq!(json_of(actual), json_of(expected));
    }

    fn write_nested_file(
        dir: &TempDir,
        name: &str,
        contents: &str,
    ) -> PathBuf {
        let path: PathBuf = dir.path().join(name);

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("parent dir creatable");
        }

        write_file(dir, name, contents);

        path
    }

    #[test]
    fn test_value_mode_clones_provided_map() {
        let map: SourceMap = sample_map("original.ts");

        let resolved: Option<SourceMap> = resolve_input_source_map(
            "",
            Path::new(""),
            "index.ts",
            &InputSourceMap::Mode(InputSourceMapMode::Value(map.clone())),
        )
        .expect("value mode always succeeds");

        assert_same_map(&resolved.expect("map present"), &map);
    }

    #[test]
    fn test_options_value_mode_clones_provided_map_and_ignores_path() {
        let map: SourceMap = sample_map("original.ts");

        let resolved: Option<SourceMap> = resolve_input_source_map(
            "",
            Path::new(""),
            "index.ts",
            &InputSourceMap::Options(InputSourceMapOptions {
                mode: InputSourceMapMode::Value(map.clone()),
                path: Some(PathBuf::from("ignored.map")),
            }),
        )
        .expect("value options always succeed");

        assert_same_map(&resolved.expect("map present"), &map);
    }

    #[test]
    fn test_inline_mode_extracts_comment() {
        let map: SourceMap = sample_map("original.ts");

        let code: String =
            append_inline_source_map(String::from("const a = 1;"), &map);

        let resolved: Option<SourceMap> = resolve_input_source_map(
            &code,
            Path::new(""),
            "index.ts",
            &InputSourceMap::Mode(InputSourceMapMode::Inline),
        )
        .expect("valid inline map");

        assert_same_map(&resolved.expect("map present"), &map);
    }

    #[test]
    fn test_inline_mode_absent_is_none() {
        let resolved: Option<SourceMap> = resolve_input_source_map(
            "const a = 1;",
            Path::new(""),
            "index.ts",
            &InputSourceMap::Mode(InputSourceMapMode::Inline),
        )
        .expect("absent inline is silent");

        assert!(resolved.is_none());
    }

    #[test]
    fn test_inline_mode_corrupt_is_err() {
        let code: &str = "const a = 1;\n//# sourceMappingURL=data:application/json;base64,!!!bad!!!\n";

        let result: Result<Option<SourceMap>, CompileError> =
            resolve_input_source_map(
                code,
                Path::new(""),
                "index.ts",
                &InputSourceMap::Mode(InputSourceMapMode::Inline),
            );

        assert!(result.is_err());
    }

    #[test]
    fn test_external_mode_uses_relative_path() {
        let dir: TempDir = temp_dir();
        let map: SourceMap = sample_map("original.ts");

        write_file(&dir, "custom.map", &json_of(&map));

        let resolved: Option<SourceMap> = resolve_input_source_map(
            "",
            &path_buf(&dir),
            "index.ts",
            &InputSourceMap::Options(InputSourceMapOptions {
                mode: InputSourceMapMode::External,
                path: Some(PathBuf::from("custom.map")),
            }),
        )
        .expect("relative path resolves");

        assert_same_map(&resolved.expect("map present"), &map);
    }

    #[test]
    fn test_external_mode_resolves_relative_path_against_source_dir() {
        let dir: TempDir = temp_dir();
        let map: SourceMap = sample_map("original.ts");

        write_nested_file(&dir, "src/custom.map", &json_of(&map));

        let resolved: Option<SourceMap> = resolve_input_source_map(
            "",
            &path_buf(&dir),
            "src/index.ts",
            &InputSourceMap::Options(InputSourceMapOptions {
                mode: InputSourceMapMode::External,
                path: Some(PathBuf::from("custom.map")),
            }),
        )
        .expect("source-relative path resolves");

        assert_same_map(&resolved.expect("map present"), &map);
    }

    #[test]
    fn test_external_mode_resolves_parent_relative_path_against_source_dir() {
        let dir: TempDir = temp_dir();
        let map: SourceMap = sample_map("original.ts");

        write_nested_file(&dir, "maps/custom.map", &json_of(&map));
        std::fs::create_dir_all(dir.path().join("src"))
            .expect("source dir creatable");

        let resolved: Option<SourceMap> = resolve_input_source_map(
            "",
            &path_buf(&dir),
            "src/index.ts",
            &InputSourceMap::Options(InputSourceMapOptions {
                mode: InputSourceMapMode::External,
                path: Some(PathBuf::from("../maps/custom.map")),
            }),
        )
        .expect("parent-relative path resolves");

        assert_same_map(&resolved.expect("map present"), &map);
    }

    #[test]
    fn test_external_mode_prefers_source_relative_path_over_cwd() {
        let dir: TempDir = temp_dir();
        let cwd_map: SourceMap = sample_map("cwd.ts");
        let source_map: SourceMap = sample_map("source.ts");

        write_file(&dir, "custom.map", &json_of(&cwd_map));
        write_nested_file(&dir, "src/custom.map", &json_of(&source_map));

        let resolved: Option<SourceMap> = resolve_input_source_map(
            "",
            &path_buf(&dir),
            "src/index.ts",
            &InputSourceMap::Options(InputSourceMapOptions {
                mode: InputSourceMapMode::External,
                path: Some(PathBuf::from("custom.map")),
            }),
        )
        .expect("source-relative path wins");

        assert_same_map(&resolved.expect("map present"), &source_map);
    }

    #[test]
    fn test_external_mode_empty_stripped_file_falls_back_to_cwd() {
        let dir: TempDir = temp_dir();
        let map: SourceMap = sample_map("original.ts");

        write_file(&dir, "custom.map", &json_of(&map));

        let resolved: Option<SourceMap> = resolve_input_source_map(
            "",
            &path_buf(&dir),
            "?v=1",
            &InputSourceMap::Options(InputSourceMapOptions {
                mode: InputSourceMapMode::External,
                path: Some(PathBuf::from("custom.map")),
            }),
        )
        .expect("empty file falls back to cwd");

        assert_same_map(&resolved.expect("map present"), &map);
    }

    #[test]
    fn test_external_mode_uses_absolute_path() {
        let dir: TempDir = temp_dir();
        let map: SourceMap = sample_map("original.ts");

        let map_path: PathBuf = write_file(&dir, "custom.map", &json_of(&map));

        let resolved: Option<SourceMap> = resolve_input_source_map(
            "",
            Path::new("/some/other/cwd"),
            "index.ts",
            &InputSourceMap::Options(InputSourceMapOptions {
                mode: InputSourceMapMode::External,
                path: Some(map_path),
            }),
        )
        .expect("absolute path resolves");

        assert_same_map(&resolved.expect("map present"), &map);
    }

    #[test]
    fn test_external_mode_missing_path_is_err() {
        let dir: TempDir = temp_dir();

        let result: Result<Option<SourceMap>, CompileError> =
            resolve_input_source_map(
                "",
                &path_buf(&dir),
                "index.ts",
                &InputSourceMap::Options(InputSourceMapOptions {
                    mode: InputSourceMapMode::External,
                    path: Some(PathBuf::from("absent.map")),
                }),
            );

        assert!(result.is_err());
    }

    #[test]
    fn test_external_mode_reads_file_next_to_source() {
        let dir: TempDir = temp_dir();
        let map: SourceMap = sample_map("original.ts");

        write_file(&dir, "index.ts.map", &json_of(&map));

        let resolved: Option<SourceMap> = resolve_input_source_map(
            "",
            &path_buf(&dir),
            "index.ts",
            &InputSourceMap::Mode(InputSourceMapMode::External),
        )
        .expect("external map resolves");

        assert_same_map(&resolved.expect("map present"), &map);
    }

    #[test]
    fn test_external_mode_reads_absolute_file_ignoring_cwd() {
        let dir: TempDir = temp_dir();
        let map: SourceMap = sample_map("original.ts");

        write_file(&dir, "index.ts.map", &json_of(&map));

        let file_path: PathBuf = dir.path().join("index.ts");
        let file: &str = file_path.to_str().expect("temp path is utf-8");

        let resolved: Option<SourceMap> = resolve_input_source_map(
            "",
            Path::new("/some/other/cwd"),
            file,
            &InputSourceMap::Mode(InputSourceMapMode::External),
        )
        .expect("absolute file resolves");

        assert_same_map(&resolved.expect("map present"), &map);
    }

    #[test]
    fn test_external_mode_strips_suffix_from_file() {
        let dir: TempDir = temp_dir();
        let map: SourceMap = sample_map("original.ts");

        write_file(&dir, "entry.ts.map", &json_of(&map));

        let resolved: Option<SourceMap> = resolve_input_source_map(
            "",
            &path_buf(&dir),
            "entry.ts?v=1#h",
            &InputSourceMap::Mode(InputSourceMapMode::External),
        )
        .expect("suffixed external map resolves");

        assert_same_map(&resolved.expect("map present"), &map);
    }

    #[test]
    fn test_external_mode_missing_is_err() {
        let dir: TempDir = temp_dir();

        let result: Result<Option<SourceMap>, CompileError> =
            resolve_input_source_map(
                "",
                &path_buf(&dir),
                "index.ts",
                &InputSourceMap::Mode(InputSourceMapMode::External),
            );

        assert!(result.is_err());
    }

    #[test]
    fn test_external_mode_missing_suffixed_file_names_stripped_path() {
        let dir: TempDir = temp_dir();

        let result: Result<Option<SourceMap>, CompileError> =
            resolve_input_source_map(
                "",
                &path_buf(&dir),
                "entry.ts?v=1#h",
                &InputSourceMap::Mode(InputSourceMapMode::External),
            );

        let error: CompileError =
            result.expect_err("file with suffix has no external map");

        let message: String = error.to_string();

        assert!(message.contains("entry.ts.map"), "{message}");
        assert!(!message.contains("?v=1#h"), "{message}");
    }

    #[test]
    fn test_external_mode_invalid_json_is_err() {
        let dir: TempDir = temp_dir();

        write_file(&dir, "index.ts.map", "{ not valid json");

        let result: Result<Option<SourceMap>, CompileError> =
            resolve_input_source_map(
                "",
                &path_buf(&dir),
                "index.ts",
                &InputSourceMap::Mode(InputSourceMapMode::External),
            );

        assert!(result.is_err());
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    #[test]
    fn test_external_mode_reads_map_from_non_utf8_cwd() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;

        let dir: TempDir = temp_dir();
        let map: SourceMap = sample_map("original.ts");

        let cwd: PathBuf = dir.path().join(OsStr::from_bytes(b"cwd-\xFF"));

        std::fs::create_dir(&cwd).expect("non-utf8 dir creatable");
        std::fs::write(cwd.join("index.ts.map"), json_of(&map))
            .expect("map writable");

        let resolved: Option<SourceMap> = resolve_input_source_map(
            "",
            &cwd,
            "index.ts",
            &InputSourceMap::Mode(InputSourceMapMode::External),
        )
        .expect("non-utf8 cwd resolves");

        assert_same_map(&resolved.expect("map present"), &map);
    }

    #[test]
    fn test_auto_mode_inline_wins() {
        let dir: TempDir = temp_dir();
        let inline_map: SourceMap = sample_map("inline.ts");
        let external_map: SourceMap = sample_map("external.ts");

        write_file(&dir, "index.ts.map", &json_of(&external_map));

        let code: String =
            append_inline_source_map(String::from("const a = 1;"), &inline_map);

        let resolved: Option<SourceMap> = resolve_input_source_map(
            &code,
            &path_buf(&dir),
            "index.ts",
            &InputSourceMap::Mode(InputSourceMapMode::Auto),
        )
        .expect("auto resolves");

        assert_same_map(&resolved.expect("map present"), &inline_map);
    }

    #[test]
    fn test_auto_mode_external_fallback() {
        let dir: TempDir = temp_dir();
        let external_map: SourceMap = sample_map("external.ts");

        write_file(&dir, "index.ts.map", &json_of(&external_map));

        let resolved: Option<SourceMap> = resolve_input_source_map(
            "const a = 1;",
            &path_buf(&dir),
            "index.ts",
            &InputSourceMap::Mode(InputSourceMapMode::Auto),
        )
        .expect("auto resolves");

        assert_same_map(&resolved.expect("map present"), &external_map);
    }

    #[test]
    fn test_auto_mode_uses_path_for_external_fallback() {
        let dir: TempDir = temp_dir();
        let external_map: SourceMap = sample_map("external.ts");

        write_file(&dir, "custom.map", &json_of(&external_map));

        let resolved: Option<SourceMap> = resolve_input_source_map(
            "const a = 1;",
            &path_buf(&dir),
            "index.ts",
            &InputSourceMap::Options(InputSourceMapOptions {
                mode: InputSourceMapMode::Auto,
                path: Some(PathBuf::from("custom.map")),
            }),
        )
        .expect("auto path resolves");

        assert_same_map(&resolved.expect("map present"), &external_map);
    }

    #[test]
    fn test_auto_mode_resolves_relative_path_against_source_dir() {
        let dir: TempDir = temp_dir();
        let external_map: SourceMap = sample_map("external.ts");

        write_nested_file(&dir, "src/custom.map", &json_of(&external_map));

        let resolved: Option<SourceMap> = resolve_input_source_map(
            "const a = 1;",
            &path_buf(&dir),
            "src/index.ts",
            &InputSourceMap::Options(InputSourceMapOptions {
                mode: InputSourceMapMode::Auto,
                path: Some(PathBuf::from("custom.map")),
            }),
        )
        .expect("auto source-relative path resolves");

        assert_same_map(&resolved.expect("map present"), &external_map);
    }

    #[test]
    fn test_auto_mode_missing_path_is_none() {
        let dir: TempDir = temp_dir();

        let resolved: Option<SourceMap> = resolve_input_source_map(
            "const a = 1;",
            &path_buf(&dir),
            "index.ts",
            &InputSourceMap::Options(InputSourceMapOptions {
                mode: InputSourceMapMode::Auto,
                path: Some(PathBuf::from("absent.map")),
            }),
        )
        .expect("auto remains best effort");

        assert!(resolved.is_none());
    }

    #[test]
    fn test_inline_mode_ignores_path() {
        let dir: TempDir = temp_dir();
        let map: SourceMap = sample_map("original.ts");

        let code: String =
            append_inline_source_map(String::from("const a = 1;"), &map);

        let resolved: Option<SourceMap> = resolve_input_source_map(
            &code,
            &path_buf(&dir),
            "index.ts",
            &InputSourceMap::Options(InputSourceMapOptions {
                mode: InputSourceMapMode::Inline,
                path: Some(PathBuf::from("absent.map")),
            }),
        )
        .expect("inline ignores the path");

        assert_same_map(&resolved.expect("map present"), &map);
    }

    #[test]
    fn test_auto_mode_strips_suffix_for_external_fallback() {
        let dir: TempDir = temp_dir();
        let external_map: SourceMap = sample_map("external.ts");

        write_file(&dir, "entry.ts.map", &json_of(&external_map));

        let resolved: Option<SourceMap> = resolve_input_source_map(
            "const a = 1;",
            &path_buf(&dir),
            "entry.ts?v=1#h",
            &InputSourceMap::Mode(InputSourceMapMode::Auto),
        )
        .expect("auto suffix resolves");

        assert_same_map(&resolved.expect("map present"), &external_map);
    }

    #[test]
    fn test_auto_mode_missing_is_none() {
        let dir: TempDir = temp_dir();

        let resolved: Option<SourceMap> = resolve_input_source_map(
            "const a = 1;",
            &path_buf(&dir),
            "index.ts",
            &InputSourceMap::Mode(InputSourceMapMode::Auto),
        )
        .expect("auto is best effort");

        assert!(resolved.is_none());
    }

    #[test]
    fn test_auto_mode_missing_suffixed_file_is_none() {
        let dir: TempDir = temp_dir();

        let resolved: Option<SourceMap> = resolve_input_source_map(
            "const a = 1;",
            &path_buf(&dir),
            "entry.ts?v=1#h",
            &InputSourceMap::Mode(InputSourceMapMode::Auto),
        )
        .expect("file with suffix is best effort");

        assert!(resolved.is_none());
    }

    #[test]
    fn test_auto_mode_corrupt_inline_is_err() {
        let dir: TempDir = temp_dir();
        let external_map: SourceMap = sample_map("external.ts");

        write_file(&dir, "index.ts.map", &json_of(&external_map));

        let code: &str = "const a = 1;\n//# sourceMappingURL=data:application/json;base64,!!!bad!!!\n";

        let result: Result<Option<SourceMap>, CompileError> =
            resolve_input_source_map(
                code,
                &path_buf(&dir),
                "index.ts",
                &InputSourceMap::Mode(InputSourceMapMode::Auto),
            );

        assert!(result.is_err());
    }
}
