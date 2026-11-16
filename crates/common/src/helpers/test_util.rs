use std::fs;
use std::path::PathBuf;

use oxc::ast::ast::{Comment, Expression, Statement};
use tempfile::TempDir;

use crate::ast::ast::Ast;

pub fn temp_dir() -> TempDir {
    tempfile::tempdir().expect("temp dir creatable")
}

pub fn path_buf(dir: &TempDir) -> PathBuf {
    dir.path().to_path_buf()
}

pub fn write_file(
    dir: &TempDir,
    name: &str,
    contents: &str,
) -> PathBuf {
    let path: PathBuf = dir.path().join(name);

    fs::write(&path, contents).expect("temp file writable");

    path
}

pub fn comment_text(
    ast: &Ast,
    comment: &Comment,
) -> String {
    let start: usize = comment.span.start as usize;
    let end: usize = comment.span.end as usize;

    ast.source()[start..end].to_string()
}

pub fn first_initializer(ast: &Ast) -> &Expression<'_> {
    let Statement::VariableDeclaration(declaration) = &ast.program().body[0]
    else {
        panic!("expected a variable declaration");
    };

    declaration
        .declarations
        .first()
        .and_then(|declarator| declarator.init.as_ref())
        .expect("declaration has an initializer")
}
