use telarel_common::Ast;

use crate::_types::sourcemap::JsSourceMap;
use crate::ast;

/// The JS-facing normalized AST: the ESTree program, the source text it spans
/// anchor to, an optional source map for the input, and the grammar label.
#[napi_derive::napi(object)]
pub struct JsAst {
    /// The ESTree program that parsing or rewriting produces. It carries the
    /// top-level `comments` array when the AST has comments.
    pub program: serde_json::Value,
    /// Source code text.
    pub code: String,
    /// The input source map, when the AST carries one.
    pub map: Option<JsSourceMap>,
    /// The grammar of the program, as its canonical label.
    pub language: String,
}

impl JsAst {
    /// Convert an owned [`Ast`] into its JS-facing form.
    pub fn from_ast(ast: &Ast) -> anyhow::Result<Self> {
        Ok(ast::ast_to_js(ast)?.into())
    }
}

impl From<ast::JsAst> for JsAst {
    fn from(value: ast::JsAst) -> Self {
        Self {
            program: value.program,
            code: value.code,
            map: value.map,
            language: value.language,
        }
    }
}

/// The JS-facing input form of the normalized AST. This is the read side of
/// [`JsAst`].
#[napi_derive::napi(object)]
pub struct JsAstInput {
    /// The ESTree program supplied as input. A top-level `comments` array
    /// preserves comments, and an absent array means no comments.
    pub program: serde_json::Value,
    /// Source code text.
    pub code: String,
    /// The source map object for the input, when the caller supplied one.
    pub map: Option<serde_json::Value>,
    /// The grammar of the program. Absent means unset.
    pub language: Option<String>,
}

impl From<JsAstInput> for ast::JsAstInput {
    fn from(value: JsAstInput) -> Self {
        Self {
            program: value.program,
            code: value.code,
            map: value.map,
            language: value.language,
        }
    }
}

/// Result of the AST-returning entry points (`parse`, `rewrite`) and of a
/// `parse` hook replacement.
#[napi_derive::napi(object)]
pub struct JsAstResult {
    /// Parsed or rewritten AST.
    pub ast: JsAst,
}

impl JsAstResult {
    /// Convert an owned [`Ast`] into the JS-facing result.
    pub fn from_ast(ast: &Ast) -> anyhow::Result<Self> {
        Ok(Self { ast: JsAst::from_ast(ast)? })
    }
}
