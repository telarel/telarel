pub mod builtin;
pub mod functions;
pub mod output;
pub mod resolvable;
pub mod resolved;
pub mod sourcemap;

/// The full raw oxc parser options used by resolved and low-level values.
pub type OxcParseOptions = oxc::parser::ParseOptions;

/// The full raw oxc codegen options used by resolved and low-level values.
pub type OxcCodegenOptions = oxc::codegen::CodegenOptions;
