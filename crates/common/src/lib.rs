//! # Telarel Common
//!
//! A shared library for the compiler.
//!
//! This crate provides the shared types and utilities for the compiler.

mod _types;
mod ast;
mod contexts;
mod errors;
mod sourcemap;

pub use crate::_types::hooks::usage::HookUsage;
pub use crate::_types::options::compile::CompileOptions;
pub use crate::_types::options::language::Language;
pub use crate::_types::options::language::grammar_source_type;
pub use crate::_types::options::resolved::ResolvedOptions;
pub use crate::_types::options::source_type::SourceType;
pub use crate::_types::options::source_type::with_module_kind;
pub use crate::ast::ast::Ast;
pub use crate::ast::codegen::{CodegenOptions, CodegenResult, codegen};
pub use crate::ast::parse::{
    ParseOptions, ParseOwnedOptions, ParseResult, parse, parse_owned,
    source_type_for_module_id,
};
pub use crate::contexts::compile::CompileContext;
pub use crate::errors::compile::CompileError;
pub use crate::sourcemap::{SourceMap, compose::compose_maps};
