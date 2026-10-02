use crate::_types::sourcemap::SourceMap;

/// Arguments for the `post` hook.
#[derive(Debug, Clone, Copy)]
pub struct PostArgs<'a> {
    /// The code that was compiled.
    pub code: &'a str,
}

/// Result of the `post` hook: the updated code and an optional incremental map.
#[derive(Debug, Clone)]
pub struct PostOutput {
    /// The updated code.
    pub code: String,
    /// The updated source map, relative to the generated code this hook received.
    pub map: Option<SourceMap>,
}

/// The `post` hook return: replacement code with an optional map, `None`, or an error.
pub type PostReturn = anyhow::Result<Option<PostOutput>>;

#[cfg(test)]
mod tests {
    use oxc_sourcemap::SourceMapBuilder;

    use super::*;

    #[test]
    fn test_post_output_without_map() {
        let output: PostOutput =
            PostOutput { code: String::from("let a;"), map: None };

        assert_eq!(output.code, "let a;");
        assert!(output.map.is_none());
    }

    #[test]
    fn test_post_output_with_map() {
        let mut builder: SourceMapBuilder<'_> = SourceMapBuilder::default();

        builder.set_file("a.ts");

        let source_id: u32 = builder.add_source_and_content("a.ts", "let a;");

        builder.add_token(0, 0, 0, 0, Some(source_id), None);

        let map: SourceMap = builder.into_owned_sourcemap().into_inner();

        let output: PostOutput =
            PostOutput { code: String::from("let a;"), map: Some(map) };

        assert_eq!(output.code, "let a;");
        assert_eq!(
            output.map.as_ref().and_then(|m| m.get_source(0)),
            Some("a.ts")
        );
    }
}
