use std::path::Path;

use telarel_common::Language;
use telarel_common::strip_file_suffix;

/// Declaration files emit no runtime code, so their suffix must survive every
/// grammar mapping.
fn is_declaration(path: &str) -> bool {
    path.ends_with(".d.ts")
        || path.ends_with(".d.mts")
        || path.ends_with(".d.cts")
}

/// The path's final extension, if it has one.
fn extension(path: &str) -> Option<&str> {
    Path::new(path).extension().and_then(|extension| extension.to_str())
}

/// Append an extension to an extensionless path.
fn append_extension(
    path: &str,
    extension: &str,
) -> String {
    format!("{path}.{extension}")
}

/// Replace the path's final extension.
fn replace_extension(
    path: &str,
    extension: &str,
) -> String {
    Path::new(path).with_extension(extension).to_string_lossy().into_owned()
}

/// Map a TypeScript file to its JavaScript output extension.
///
/// `Language` collapses `.ts`, `.mts`, and `.cts` into `TS`, so the suffix
/// decides the module flavor.
fn typescript_output(path: &str) -> String {
    if is_declaration(path) {
        return path.to_string();
    }

    match extension(path) {
        | None => append_extension(path, "js"),
        | Some("ts") => replace_extension(path, "js"),
        | Some("mts") => replace_extension(path, "mjs"),
        | Some("cts") => replace_extension(path, "cjs"),
        | Some(_) => path.to_string(),
    }
}

/// Map a JSX-bearing file to `.jsx` when preserved and `.js` when lowered.
fn jsx_output(
    path: &str,
    jsx_preserved: bool,
) -> String {
    if is_declaration(path) {
        return path.to_string();
    }

    let extension_out: &str = if jsx_preserved { "jsx" } else { "js" };

    match extension(path) {
        | None => append_extension(path, extension_out),
        | Some("tsx" | "jsx" | "ts" | "js" | "mjs" | "cjs") => {
            replace_extension(path, extension_out)
        },
        | Some(_) => path.to_string(),
    }
}

/// Derive the output file name from `file`, its effective grammar, and whether
/// JSX is preserved.
///
/// Returns `None` when the derived name would equal `file`. The `?query` and
/// `#hash` suffixes are stripped before suffix matching and never appear in a
/// derived name.
pub fn derive_output_file(
    file: &str,
    language: Language,
    jsx_preserved: bool,
) -> Option<String> {
    let path: &str = strip_file_suffix(file);

    let derived: String = match language {
        | Language::TS => typescript_output(path),
        | Language::TSX | Language::JSX => jsx_output(path, jsx_preserved),
        | Language::JS | Language::DTS => path.to_string(),
    };

    (derived != file).then_some(derived)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_derived(
        file: &str,
        language: Language,
        jsx_preserved: bool,
        expected: Option<&str>,
    ) {
        assert_eq!(
            derive_output_file(file, language, jsx_preserved).as_deref(),
            expected,
            "{file}",
        );
    }

    #[test]
    fn test_derive_typescript_suffixes() {
        assert_derived("index.ts", Language::TS, false, Some("index.js"));
        assert_derived("index.mts", Language::TS, false, Some("index.mjs"));
        assert_derived("index.cts", Language::TS, false, Some("index.cjs"));
    }

    #[test]
    fn test_derive_typescript_keeps_javascript_module_flavors() {
        assert_derived("index.mjs", Language::TS, false, None);
        assert_derived("index.cjs", Language::TS, false, None);
    }

    #[test]
    fn test_derive_typescript_appends_to_extensionless_files() {
        assert_derived("index", Language::TS, false, Some("index.js"));
        assert_derived("src/index", Language::TS, false, Some("src/index.js"));
    }

    #[test]
    fn test_derive_typescript_keeps_unknown_suffixes() {
        assert_derived("index.css", Language::TS, false, None);
        assert_derived("index.jsx", Language::TS, false, None);
    }

    #[test]
    fn test_derive_keeps_declaration_suffixes() {
        for language in [Language::TS, Language::TSX, Language::JSX] {
            for jsx_preserved in [false, true] {
                for file in ["index.d.ts", "index.d.mts", "index.d.cts"] {
                    assert_derived(file, language, jsx_preserved, None);
                }
            }
        }
    }

    #[test]
    fn test_derive_jsx_preserved_maps_known_suffixes_to_jsx() {
        let cases: [(&str, &str); 5] = [
            ("index.tsx", "index.jsx"),
            ("index.ts", "index.jsx"),
            ("index.js", "index.jsx"),
            ("index.mjs", "index.jsx"),
            ("index.cjs", "index.jsx"),
        ];

        for (file, expected) in cases {
            assert_derived(file, Language::TSX, true, Some(expected));
        }

        assert_derived("index.jsx", Language::TSX, true, None);
        assert_derived("index.jsx", Language::JSX, true, None);
    }

    #[test]
    fn test_derive_jsx_lowered_maps_known_suffixes_to_js() {
        let cases: [(&str, &str); 5] = [
            ("index.tsx", "index.js"),
            ("index.jsx", "index.js"),
            ("index.ts", "index.js"),
            ("index.mjs", "index.js"),
            ("index.cjs", "index.js"),
        ];

        for (file, expected) in cases {
            assert_derived(file, Language::TSX, false, Some(expected));
        }

        assert_derived("index.js", Language::TSX, false, None);
        assert_derived("index.js", Language::JSX, false, None);
    }

    #[test]
    fn test_derive_jsx_appends_to_extensionless_files() {
        assert_derived("index", Language::TSX, true, Some("index.jsx"));
        assert_derived("index", Language::TSX, false, Some("index.js"));
        assert_derived("index", Language::JSX, true, Some("index.jsx"));
        assert_derived("index", Language::JSX, false, Some("index.js"));
    }

    #[test]
    fn test_derive_jsx_keeps_unknown_suffixes() {
        assert_derived("index.css", Language::TSX, true, None);
        assert_derived("index.css", Language::TSX, false, None);
        assert_derived("index.mts", Language::TSX, true, None);
        assert_derived("index.cts", Language::JSX, false, None);
    }

    #[test]
    fn test_derive_javascript_and_declarations_are_unchanged() {
        assert_derived("index.ts", Language::JS, false, None);
        assert_derived("index.tsx", Language::JS, false, None);
        assert_derived("index.ts", Language::DTS, false, None);
        assert_derived("index.d.ts", Language::DTS, false, None);
    }

    #[test]
    fn test_derive_strips_query_and_hash() {
        assert_derived("index.ts?v=1", Language::TS, false, Some("index.js"));
        assert_derived(
            "index.tsx?v=1#hash",
            Language::TSX,
            true,
            Some("index.jsx"),
        );
        assert_derived(
            "index.tsx#hash",
            Language::TSX,
            false,
            Some("index.js"),
        );
    }

    #[test]
    fn test_derive_strips_suffixes_from_otherwise_unchanged_names() {
        assert_derived("index.mjs?v=1", Language::TS, false, Some("index.mjs"));
        assert_derived(
            "index.css#hash",
            Language::TS,
            false,
            Some("index.css"),
        );
    }
}
