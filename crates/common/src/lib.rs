//! # Telarel Common
//!
//! A shared library for the compiler.
//!
//! This crate provides the shared types and utilities for the compiler.

mod _types;
mod ast;
mod contexts;
mod errors;
mod helpers;
mod sourcemap;

pub use crate::_types::grammar::language::Language;
pub use crate::_types::grammar::language::grammar_source_type;
pub use crate::_types::grammar::source_type::SourceType;
pub use crate::_types::grammar::source_type::with_module_kind;
pub use crate::_types::hooks::usage::HookUsage;
pub use crate::_types::options::builtin::generator::GeneratorOptions;
pub use crate::_types::options::builtin::parser::ParserOptions;
pub use crate::_types::options::functions::compile::CompileOptions;
pub use crate::_types::options::functions::compile_from_ast::CompileFromAstOptions;
pub use crate::_types::options::functions::generate::GenerateOptions;
pub use crate::_types::options::functions::parse::ParseOptions;
pub use crate::_types::options::functions::rewrite::RewriteOptions;
pub use crate::_types::options::resolvable::ResolvableOptions;
pub use crate::_types::options::resolved::ResolvedOptions;
pub use crate::_types::options::source_map::SourceMapOptions;
pub use crate::_types::options::source_map::input::{
    InputSourceMap, InputSourceMapMode,
};
pub use crate::_types::options::source_map::output::{
    OutputSourceMap, OutputSourceMapMode, OutputSourceMapOptions,
};
pub use crate::ast::ast::Ast;
pub use crate::ast::generate::{
    GenerateCodeOptions, generate_code, generate_program,
};
pub use crate::ast::generator::Generator;
pub use crate::ast::parse::{
    ParseAstOptions, ParseProgramOptions, ParseProgramResult, parse_ast,
    parse_program,
};
pub use crate::ast::parser::Parser;
pub use crate::ast::result::{AstResult, CodeResult};
pub use crate::contexts::compile::CompileContext;
pub use crate::errors::compile::CompileError;
pub use crate::helpers::resolve::{
    from_oxc_language, from_oxc_module_kind, infer_language, resolve_grammar,
    resolve_options, resolve_source_type, source_type_for_module_id,
};
pub use crate::sourcemap::inline::{
    append_inline_source_map, extract_inline_source_map,
};
pub use crate::sourcemap::mode::apply_output_source_map_mode;
pub use crate::sourcemap::resolve::resolve_input_source_map;
pub use crate::sourcemap::{SourceMap, compose::compose_maps};
