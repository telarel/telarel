use telarel_common::Ast;

use crate::_types::sourcemap::JsSourceMap;
use crate::ast;

/// The JS-facing normalized AST: the ESTree program, the source text it spans
/// anchor to, and an optional input source map.
#[napi_derive::napi(object)]
pub struct JsAst {
    /// The ESTree program.
    pub program: serde_json::Value,
    /// The source text the program spans anchor to.
    pub code: String,
    /// The input source map, when the AST carries one.
    pub map: Option<JsSourceMap>,
}

impl JsAst {
    /// Convert an owned [`Ast`] into its JS-facing form.
    pub fn from_ast(ast: &Ast) -> anyhow::Result<Self> {
        Ok(ast::ast_to_js(ast)?.into())
    }
}

impl From<ast::JsAst> for JsAst {
    fn from(value: ast::JsAst) -> Self {
        Self { program: value.program, code: value.code, map: value.map }
    }
}

/// The JS-facing normalized AST input, the read side of [`JsAst`].
#[napi_derive::napi(object)]
pub struct JsAstInput {
    /// The ESTree program.
    pub program: serde_json::Value,
    /// The source text the program spans anchor to.
    pub code: String,
    /// The input source map object, when the caller supplied one.
    pub map: Option<serde_json::Value>,
}

impl From<JsAstInput> for ast::JsAstInput {
    fn from(value: JsAstInput) -> Self {
        Self { program: value.program, code: value.code, map: value.map }
    }
}

/// Result of the AST-returning entry points (`parse`, `rewrite`) and of a
/// `parse` hook replacement.
#[napi_derive::napi(object)]
pub struct JsAstResult {
    /// The parsed or rewritten AST.
    pub ast: JsAst,
}

impl JsAstResult {
    /// Convert an owned [`Ast`] into the JS-facing result.
    pub fn from_ast(ast: &Ast) -> anyhow::Result<Self> {
        Ok(Self { ast: JsAst::from_ast(ast)? })
    }
}
