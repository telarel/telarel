//! # Telarel Common
//!
//! A shared library for the compiler.
//!
//! This crate provides the shared types and utilities for the compiler.

mod _types;
mod ast;
mod contexts;
mod errors;

pub use _types::hooks::usage::HookUsage;
pub use _types::options::compile::CompileOptions;
pub use _types::options::language::Language;
pub use _types::options::source_type::SourceType;
pub use ast::codegen::{CodegenOptions, CodegenResult, codegen};
pub use ast::parse::{ParseOptions, ParseResult, parse};
pub use contexts::compile::CompileContext;
pub use errors::compile::CompileError;
