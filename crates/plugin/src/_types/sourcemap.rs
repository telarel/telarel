/// An owned source map crossing hook boundaries;
/// owned by `common` and re-exported here (the plugin-surface alias).
pub use telarel_common::SourceMap;

#[cfg(test)]
mod tests {
    use oxc_sourcemap::SourceMapBuilder;

    use super::*;

    #[test]
    fn test_sourcemap_alias_is_owned_static_map() {
        let mut builder: SourceMapBuilder<'_> = SourceMapBuilder::default();

        builder.set_file("a.ts");

        let source_id: u32 = builder.add_source_and_content("a.ts", "let a;");

        builder.add_token(0, 0, 0, 0, Some(source_id), None);

        let map: SourceMap = builder.into_owned_sourcemap().into_inner();

        assert_eq!(map.get_source(0), Some("a.ts"));
    }
}
