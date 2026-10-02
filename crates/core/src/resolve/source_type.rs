use telarel_common::{Language, SourceType, with_module_kind};

use crate::resolve::grammar::{from_oxc_module_kind, resolve_grammar};

/// Resolve the telarel source type for a file, mirroring `parse`'s resolution:
/// the resolved grammar's module kind, overlaid by an explicit option.
/// The resolved value is the module kind the parser will see.
pub fn resolve_source_type(
    file: &str,
    language: Option<Language>,
    source_type: Option<SourceType>,
) -> SourceType {
    let grammar: oxc::span::SourceType = resolve_grammar(file, language);

    let source_type: oxc::span::SourceType = match source_type {
        | Some(module_kind) => with_module_kind(grammar, module_kind),
        | None => grammar,
    };

    from_oxc_module_kind(source_type)
}

#[cfg(test)]
mod tests {
    use crate::resolve::language::infer_language;

    use super::*;

    #[test]
    fn test_resolve_source_type_keeps_grammar_module_kind_without_option() {
        // An explicit language yields an unambiguous grammar: its module
        // kind is telarel's `Unambiguous`, mirrored from the parser.
        let source_type: SourceType =
            resolve_source_type("index.ts", Some(Language::TS), None);

        assert_eq!(source_type, SourceType::Unambiguous);

        // Without an explicit language, the module id resolves the kind.
        let source_type: SourceType =
            resolve_source_type("index.ts", None, None);

        assert_eq!(source_type, SourceType::Unambiguous);

        let source_type: SourceType =
            resolve_source_type("index.mjs", None, None);

        assert_eq!(source_type, SourceType::Module);
    }

    #[test]
    fn test_resolve_source_type_explicit_module_kind_wins() {
        let source_type: SourceType =
            resolve_source_type("index.ts", None, Some(SourceType::Script));

        assert_eq!(source_type, SourceType::Script);
    }

    #[test]
    fn test_resolve_source_type_cjs_extension_resolves_commonjs() {
        let language: Language = infer_language("index.cjs", None);

        assert_eq!(language, Language::JS);

        let source_type: SourceType =
            resolve_source_type("index.cjs", None, None);

        assert_eq!(source_type, SourceType::CommonJS);
    }
}
