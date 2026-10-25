//! # Telarel Core
//!
//! A pipeline orchestrator for the compiler.
//!
//! The crate exposes five entry points over one shared skeleton: `parse`,
//! `rewrite`, `generate`, `compile_from_ast`, and `compile`. Each converts its
//! options to [`ResolvableOptions`](telarel_common::ResolvableOptions), runs
//! the `options` fixpoint, resolves the settled options, runs `start`, executes
//! the stages its entry point declares, and runs `end`. The pipeline invokes
//! each stage once per invocation; `options` runs per plugin instance, while
//! `start` and `end` run per settled entry, duplicates included.

mod common;
mod functions;

pub use crate::functions::compile::compile;
pub use crate::functions::compile_from_ast::compile_from_ast;
pub use crate::functions::generate::generate;
pub use crate::functions::parse::parse;
pub use crate::functions::rewrite::rewrite;
