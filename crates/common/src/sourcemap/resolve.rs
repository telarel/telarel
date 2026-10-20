use std::path::{Path, PathBuf};

use crate::_types::options::source_map::input::{
    InputSourceMap, InputSourceMapMode,
};
use crate::errors::compile::CompileError;
use crate::sourcemap::SourceMap;
use crate::sourcemap::inline::extract_inline_source_map;

/// Resolve the input source map requested by `input`.
///
/// `code` is the source being parsed (`Inline`/`Auto` scan it for an inline
/// comment); `cwd` and `file` locate external map files (`External` reads
/// `<cwd>/<file>.map`, `Path` resolves relative paths against `cwd`).
///
/// Returns `Ok(None)` only when the mode is best-effort (`Inline`/`Auto`) and
/// no map was found. Explicitly requested but missing maps (`Path`/`External`)
/// and corrupt data are `Err`.
pub fn resolve_input_source_map(
    code: &str,
    cwd: &str,
    file: &str,
    input: &InputSourceMap,
) -> Result<Option<SourceMap>, CompileError> {
    let InputSourceMap::Mode(mode) = input;

    resolve_input_source_map_mode(code, cwd, file, mode)
}

/// Resolve the map for a concrete [`InputSourceMapMode`].
fn resolve_input_source_map_mode(
    code: &str,
    cwd: &str,
    file: &str,
    mode: &InputSourceMapMode,
) -> Result<Option<SourceMap>, CompileError> {
    match mode {
        | InputSourceMapMode::Value(map) => Ok(Some(map.clone())),
        | InputSourceMapMode::Inline => extract_inline_source_map(code)
            .map_or(Ok(None), |result| result.map(Some)),
        | InputSourceMapMode::Path(path) => {
            let resolved: PathBuf = resolve_path(cwd, path);
            let map: SourceMap = read_map_file(&resolved)?;

            Ok(Some(map))
        },
        | InputSourceMapMode::External => {
            let path: PathBuf = external_map_path(cwd, file);
            let map: SourceMap = read_map_file(&path)?;

            Ok(Some(map))
        },
        | InputSourceMapMode::Auto => match extract_inline_source_map(code) {
            | Some(Ok(map)) => Ok(Some(map)),
            | Some(Err(error)) => Err(error),
            | None => {
                let path: PathBuf = external_map_path(cwd, file);

                if path.is_file() {
                    let map: SourceMap = read_map_file(&path)?;

                    Ok(Some(map))
                } else {
                    Ok(None)
                }
            },
        },
    }
}

/// Resolve `path` against `cwd` unless it is already absolute.
fn resolve_path(
    cwd: &str,
    path: &Path,
) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        Path::new(cwd).join(path)
    }
}

/// The external map path `<cwd>/<file>.map` for `file`.
fn external_map_path(
    cwd: &str,
    file: &str,
) -> PathBuf {
    Path::new(cwd).join(format!("{file}.map"))
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use oxc_sourcemap::SourceMapBuilder;
    use tempfile::TempDir;

    use crate::helpers::test_util::{path_string, temp_dir, write_file};
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

    #[test]
    fn test_value_mode_clones_provided_map() {
        let map: SourceMap = sample_map("original.ts");

        let resolved: Option<SourceMap> = resolve_input_source_map(
            "",
            "",
            "index.ts",
            &InputSourceMap::Mode(InputSourceMapMode::Value(map.clone())),
        )
        .expect("value mode always succeeds");

        assert_same_map(&resolved.expect("map present"), &map);
    }

    #[test]
    fn test_inline_mode_extracts_comment() {
        let map: SourceMap = sample_map("original.ts");

        let code: String =
            append_inline_source_map(String::from("const a = 1;"), &map);

        let resolved: Option<SourceMap> = resolve_input_source_map(
            &code,
            "",
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
            "",
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
                "",
                "index.ts",
                &InputSourceMap::Mode(InputSourceMapMode::Inline),
            );

        assert!(result.is_err());
    }

    #[test]
    fn test_path_mode_relative_to_cwd() {
        let dir: TempDir = temp_dir();
        let map: SourceMap = sample_map("original.ts");

        write_file(&dir, "custom.map", &json_of(&map));

        let resolved: Option<SourceMap> = resolve_input_source_map(
            "",
            &path_string(&dir),
            "index.ts",
            &InputSourceMap::Mode(InputSourceMapMode::Path(PathBuf::from(
                "custom.map",
            ))),
        )
        .expect("relative map resolves");

        assert_same_map(&resolved.expect("map present"), &map);
    }

    #[test]
    fn test_path_mode_absolute() {
        let dir: TempDir = temp_dir();
        let map: SourceMap = sample_map("original.ts");

        let map_path: PathBuf = write_file(&dir, "custom.map", &json_of(&map));

        let resolved: Option<SourceMap> = resolve_input_source_map(
            "",
            "/some/other/cwd",
            "index.ts",
            &InputSourceMap::Mode(InputSourceMapMode::Path(map_path)),
        )
        .expect("absolute map resolves");

        assert_same_map(&resolved.expect("map present"), &map);
    }

    #[test]
    fn test_path_mode_missing_is_err() {
        let dir: TempDir = temp_dir();

        let result: Result<Option<SourceMap>, CompileError> =
            resolve_input_source_map(
                "",
                &path_string(&dir),
                "index.ts",
                &InputSourceMap::Mode(InputSourceMapMode::Path(PathBuf::from(
                    "absent.map",
                ))),
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
            &path_string(&dir),
            "index.ts",
            &InputSourceMap::Mode(InputSourceMapMode::External),
        )
        .expect("external map resolves");

        assert_same_map(&resolved.expect("map present"), &map);
    }

    #[test]
    fn test_external_mode_missing_is_err() {
        let dir: TempDir = temp_dir();

        let result: Result<Option<SourceMap>, CompileError> =
            resolve_input_source_map(
                "",
                &path_string(&dir),
                "index.ts",
                &InputSourceMap::Mode(InputSourceMapMode::External),
            );

        assert!(result.is_err());
    }

    #[test]
    fn test_external_mode_invalid_json_is_err() {
        let dir: TempDir = temp_dir();

        write_file(&dir, "index.ts.map", "{ not valid json");

        let result: Result<Option<SourceMap>, CompileError> =
            resolve_input_source_map(
                "",
                &path_string(&dir),
                "index.ts",
                &InputSourceMap::Mode(InputSourceMapMode::External),
            );

        assert!(result.is_err());
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
            &path_string(&dir),
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
            &path_string(&dir),
            "index.ts",
            &InputSourceMap::Mode(InputSourceMapMode::Auto),
        )
        .expect("auto resolves");

        assert_same_map(&resolved.expect("map present"), &external_map);
    }

    #[test]
    fn test_auto_mode_missing_is_none() {
        let dir: TempDir = temp_dir();

        let resolved: Option<SourceMap> = resolve_input_source_map(
            "const a = 1;",
            &path_string(&dir),
            "index.ts",
            &InputSourceMap::Mode(InputSourceMapMode::Auto),
        )
        .expect("auto is best effort");

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
                &path_string(&dir),
                "index.ts",
                &InputSourceMap::Mode(InputSourceMapMode::Auto),
            );

        assert!(result.is_err());
    }
}
