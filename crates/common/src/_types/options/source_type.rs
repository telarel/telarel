use oxc::span::SourceType as OxcSourceType;

/// The module system / execution mode of the source code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceType {
    /// Classic non-module script.
    Script,
    /// CommonJS (`require` / `module.exports`).
    CommonJS,
    /// ES module (`import` / `export`).
    Module,
    /// The parser infers from the statements.
    Unambiguous,
}

/// Overlay a module kind onto a resolved oxc source type.
pub fn with_module_kind(
    source_type: OxcSourceType,
    module_kind: SourceType,
) -> OxcSourceType {
    let overlaid: OxcSourceType = match module_kind {
        | SourceType::Script => source_type.with_script(true),
        | SourceType::CommonJS => source_type.with_commonjs(true),
        | SourceType::Module => source_type.with_module(true),
        | SourceType::Unambiguous => source_type.with_unambiguous(true),
    };

    overlaid
}
