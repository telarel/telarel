use oxc_sourcemap::SourceMap as OxcSourceMap;

/// JS-facing source map, aligned with the standard source map v3 shape.
#[napi_derive::napi(object)]
pub struct JsSourceMap {
    /// Version of the source map format.
    pub version: u32,
    /// An optional name of the generated code that this source map is
    /// associated with.
    pub file: Option<String>,
    /// A string with the encoded mapping data.
    pub mappings: String,
    /// An optional source root, prepended to the individual entries in
    /// `sources`.
    pub source_root: Option<String>,
    /// A list of original sources used by the `mappings` entry.
    pub sources: Vec<String>,
    /// An optional list of source contents, in the same order as `sources`.
    pub sources_content: Option<Vec<Option<String>>>,
    /// A list of symbol names used by the `mappings` entry.
    pub names: Vec<String>,
    /// Indices of the `sources` entries known to be third-party code,
    /// allowing developer tools to ignore-list them.
    #[napi(js_name = "ignoreList")]
    pub ignore_list: Option<Vec<u32>>,
}

impl From<OxcSourceMap<'_>> for JsSourceMap {
    fn from(source_map: OxcSourceMap<'_>) -> Self {
        let json: oxc_sourcemap::JSONSourceMap = source_map.to_json();

        Self {
            version: 3,
            file: json.file,
            mappings: json.mappings,
            source_root: json.source_root,
            sources: json.sources,
            sources_content: json.sources_content,
            names: json.names,
            ignore_list: json.ignore_list,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn to_js_source_map(json: oxc_sourcemap::JSONSourceMap) -> JsSourceMap {
        let source_map: OxcSourceMap<'static> =
            OxcSourceMap::from_json(json).unwrap();
        JsSourceMap::from(source_map)
    }

    #[test]
    fn test_sourcemap_conversion_all_fields() {
        let json: oxc_sourcemap::JSONSourceMap = oxc_sourcemap::JSONSourceMap {
            version: 3,
            file: Some(String::from("out.js")),
            mappings: String::from("AAAA"),
            source_root: Some(String::from("/src")),
            sources: vec![String::from("index.ts")],
            sources_content: Some(vec![
                Some(String::from("const a = 1;")),
                None,
            ]),
            names: vec![String::from("a")],
            debug_id: None,
            ignore_list: Some(vec![0]),
        };
        let converted: JsSourceMap = to_js_source_map(json);

        assert_eq!(converted.version, 3_u32);
        assert_eq!(converted.file, Some(String::from("out.js")));
        assert_eq!(converted.mappings, String::from("AAAA"));
        assert_eq!(converted.source_root, Some(String::from("/src")));
        assert_eq!(converted.sources, vec![String::from("index.ts")]);
        assert_eq!(
            converted.sources_content,
            Some(vec![Some(String::from("const a = 1;")), None])
        );
        assert_eq!(converted.names, vec![String::from("a")]);
        assert_eq!(converted.ignore_list, Some(vec![0_u32]));
    }

    #[test]
    fn test_sourcemap_conversion_minimal() {
        let json: oxc_sourcemap::JSONSourceMap = oxc_sourcemap::JSONSourceMap {
            version: 3,
            file: None,
            mappings: String::new(),
            source_root: None,
            sources: Vec::new(),
            sources_content: None,
            names: Vec::new(),
            debug_id: None,
            ignore_list: None,
        };
        let converted: JsSourceMap = to_js_source_map(json);

        assert_eq!(converted.version, 3_u32);
        assert_eq!(converted.file, None);
        assert!(converted.mappings.is_empty());
        assert_eq!(converted.source_root, None);
        assert!(converted.sources.is_empty());
        assert_eq!(converted.sources_content, None);
        assert!(converted.names.is_empty());
        assert_eq!(converted.ignore_list, None);
    }

    #[test]
    fn test_sourcemap_conversion_mappings_roundtrip() {
        let mappings: &str = "AAAA,MAAM,IAAI";
        let json: oxc_sourcemap::JSONSourceMap = oxc_sourcemap::JSONSourceMap {
            version: 3,
            file: None,
            mappings: String::from(mappings),
            source_root: None,
            sources: vec![String::from("index.ts")],
            sources_content: None,
            names: Vec::new(),
            debug_id: None,
            ignore_list: None,
        };
        let source_map: OxcSourceMap<'static> =
            OxcSourceMap::from_json(json).unwrap();
        let converted: JsSourceMap = JsSourceMap::from(source_map);

        assert_eq!(converted.mappings, mappings);
        assert!(!converted.mappings.is_empty());

        let reparsed_json: String = format!(
            "{{\"version\":3,\"sources\":[\"index.ts\"],\"mappings\":\"{}\"}}",
            converted.mappings
        );
        let reparsed: OxcSourceMap<'_> =
            OxcSourceMap::from_json_string(&reparsed_json).unwrap();
        let token_count: usize = reparsed.get_tokens().count();

        assert_eq!(token_count, 3_usize);
    }

    #[test]
    fn test_sourcemap_conversion_drops_debug_id() {
        let json: oxc_sourcemap::JSONSourceMap = oxc_sourcemap::JSONSourceMap {
            version: 3,
            file: None,
            mappings: String::from("AAAA"),
            source_root: None,
            sources: vec![String::from("index.ts")],
            sources_content: None,
            names: Vec::new(),
            debug_id: Some(String::from("debug-1234")),
            ignore_list: None,
        };
        let converted: JsSourceMap = to_js_source_map(json);

        assert_eq!(converted.version, 3_u32);
        assert_eq!(converted.mappings, String::from("AAAA"));
    }

    #[test]
    fn test_sourcemap_conversion_token_details() {
        let json: oxc_sourcemap::JSONSourceMap = oxc_sourcemap::JSONSourceMap {
            version: 3,
            file: None,
            mappings: String::from("AAAA,CAEGA"),
            source_root: None,
            sources: vec![String::from("index.ts")],
            sources_content: Some(vec![Some(String::from("const a = 1;"))]),
            names: vec![String::from("a")],
            debug_id: None,
            ignore_list: None,
        };
        let source_map: OxcSourceMap<'static> =
            OxcSourceMap::from_json(json).unwrap();
        let converted: JsSourceMap = JsSourceMap::from(source_map);
        assert_eq!(converted.mappings, "AAAA,CAEGA");

        let reparsed_json: String = format!(
            "{{\"version\":3,\"sources\":[\"index.ts\"],\"names\":[\"a\"],\"mappings\":\"{}\"}}",
            converted.mappings
        );
        let reparsed: OxcSourceMap<'_> =
            OxcSourceMap::from_json_string(&reparsed_json).unwrap();
        let tokens: Vec<_> = reparsed.get_tokens().collect();
        let first: oxc_sourcemap::Token = tokens[0];
        let second: oxc_sourcemap::Token = tokens[1];

        assert_eq!(first.get_src_line(), 0_u32);
        assert_eq!(first.get_src_col(), 0_u32);
        assert_eq!(first.get_dst_line(), 0_u32);
        assert_eq!(first.get_dst_col(), 0_u32);
        assert_eq!(first.get_name_id(), None);
        assert_eq!(second.get_dst_line(), 0_u32);
        assert_eq!(second.get_dst_col(), 1_u32);
        assert_eq!(second.get_src_line(), 2_u32);
        assert_eq!(second.get_src_col(), 3_u32);
        assert_eq!(second.get_name_id(), Some(0_u32));
    }
}
