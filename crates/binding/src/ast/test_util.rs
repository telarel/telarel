use std::path::PathBuf;

use telarel_common::{Ast, Language, ParseAstOptions, SourceMap, parse_ast};

pub fn fixture_ast(code: &str) -> Ast {
    parse_ast(
        code.to_owned(),
        ParseAstOptions {
            cwd: PathBuf::from("/repo"),
            file: String::from("a.ts"),
            language: None,
            source_type: None,
            input_map: None,
        },
    )
    .expect("fixture parses")
}

pub fn parse_fixture(
    code: &str,
    language: Language,
) -> Ast {
    parse_ast(
        code.to_owned(),
        ParseAstOptions {
            cwd: PathBuf::from("/repo"),
            file: String::from("index.ts"),
            language: Some(language),
            source_type: None,
            input_map: None,
        },
    )
    .expect("fixture parses")
}

pub fn sample_map() -> SourceMap {
    let mut builder: oxc_sourcemap::SourceMapBuilder<'_> =
        oxc_sourcemap::SourceMapBuilder::default();

    builder.set_file("upstream.ts");

    let source_id: u32 =
        builder.add_source_and_content("upstream.ts", "const a = 1;");

    builder.add_token(0, 0, 0, 0, Some(source_id), None);

    builder.into_owned_sourcemap().into_inner()
}
