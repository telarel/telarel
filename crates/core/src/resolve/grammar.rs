use telarel_common::{
    Language, SourceType, grammar_source_type, source_type_for_module_id,
};

/// The oxc grammar for a file, mirroring `parse`: an explicit language wins;
/// otherwise infer from the module id.
pub fn resolve_grammar(
    file: &str,
    language: Option<Language>,
) -> oxc::span::SourceType {
    match language {
        | Some(language) => grammar_source_type(language),
        | None => source_type_for_module_id(file),
    }
}

/// Narrow an oxc source type to a internal grammar.
pub fn from_oxc_language(source_type: oxc::span::SourceType) -> Language {
    match (source_type.is_typescript(), source_type.is_jsx()) {
        | (true, true) => Language::TSX,
        | (true, false) => {
            if source_type.is_typescript_definition() {
                Language::DTS
            } else {
                Language::TS
            }
        },
        | (false, true) => Language::JSX,
        | (false, false) => Language::JS,
    }
}

/// Narrow an oxc source type to a internal module kind.
pub fn from_oxc_module_kind(source_type: oxc::span::SourceType) -> SourceType {
    if source_type.is_script() {
        SourceType::Script
    } else if source_type.is_commonjs() {
        SourceType::CommonJS
    } else if source_type.is_module() {
        SourceType::Module
    } else {
        SourceType::Unambiguous
    }
}
