use crate::_types::sourcemap::SourceMap;

/// Arguments for the `prepare` hook.
#[derive(Debug, Clone, Copy)]
pub struct PrepareArgs<'a> {
    /// The code to be compiled.
    pub code: &'a str,
}

/// Result of the `prepare` hook: the updated code and an optional incremental map.
#[derive(Debug, Clone)]
pub struct PrepareOutput {
    /// The updated code.
    pub code: String,
    /// The updated source map, relative to the code this hook received.
    pub map: Option<SourceMap>,
}

/// The `prepare` hook return: replacement code with an optional map, `None`, or an error.
pub type PrepareReturn = anyhow::Result<Option<PrepareOutput>>;

#[cfg(test)]
mod tests {
    use oxc_sourcemap::SourceMapBuilder;

    use super::*;

    #[test]
    fn test_prepare_output_without_map() {
        let output: PrepareOutput =
            PrepareOutput { code: String::from("let a;"), map: None };

        assert_eq!(output.code, "let a;");
        assert!(output.map.is_none());
    }

    #[test]
    fn test_prepare_output_with_map() {
        let mut builder: SourceMapBuilder<'_> = SourceMapBuilder::default();

        builder.set_file("a.ts");

        let source_id: u32 = builder.add_source_and_content("a.ts", "let a;");

        builder.add_token(0, 0, 0, 0, Some(source_id), None);

        let map: SourceMap = builder.into_owned_sourcemap().into_inner();

        let output: PrepareOutput =
            PrepareOutput { code: String::from("let a;"), map: Some(map) };

        assert_eq!(output.code, "let a;");
        assert_eq!(
            output.map.as_ref().and_then(|m| m.get_source(0)),
            Some("a.ts")
        );
    }
}
