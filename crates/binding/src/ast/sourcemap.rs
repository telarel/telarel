use telarel_common::SourceMap;

use crate::_types::sourcemap::JsSourceMap;

/// Convert an input source map into its JS-facing v3 object.
pub fn source_map_to_js(map: &SourceMap) -> JsSourceMap {
    JsSourceMap::from(map.clone())
}

/// Rebuild an owned [`SourceMap`] from its JS-facing v3 object.
pub fn js_source_map_to_owned(map: &JsSourceMap) -> anyhow::Result<SourceMap> {
    let json: oxc_sourcemap::JSONSourceMap = oxc_sourcemap::JSONSourceMap {
        version: map.version,
        file: map.file.clone(),
        mappings: map.mappings.clone(),
        source_root: map.source_root.clone(),
        sources: map.sources.clone(),
        sources_content: map.sources_content.clone(),
        names: map.names.clone(),
        debug_id: None,
        ignore_list: map.ignore_list.clone(),
    };

    oxc_sourcemap::SourceMap::from_json(json).map_err(anyhow::Error::msg)
}

/// Parse a source map that a hook sends as a JSON string into an owned map.
pub fn parse_json_map(json: &str) -> anyhow::Result<SourceMap> {
    Ok(oxc_sourcemap::OwnedSourceMap::from_json_string(json)
        .map_err(anyhow::Error::msg)?
        .into_inner())
}

/// Parse a JS source-map object into an owned map.
pub fn parse_map_value(value: &serde_json::Value) -> anyhow::Result<SourceMap> {
    parse_json_map(&serde_json::to_string(value)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::test_util::sample_map;

    #[test]
    fn test_js_source_map_to_owned_round_trips() {
        let map: SourceMap = sample_map();

        let js: JsSourceMap = source_map_to_js(&map);

        let rebuilt: SourceMap =
            js_source_map_to_owned(&js).expect("the map rebuilds");

        assert_eq!(rebuilt.to_json_string(), map.to_json_string());
    }

    #[test]
    fn test_parse_json_map_round_trips() {
        let map: SourceMap = sample_map();

        let parsed: SourceMap =
            parse_json_map(&map.to_json_string()).expect("map parses");

        assert_eq!(parsed.to_json_string(), map.to_json_string());
    }

    #[test]
    fn test_parse_json_map_invalid_errors() {
        assert!(parse_json_map("not json").is_err());
    }

    #[test]
    fn test_parse_map_value_round_trips() {
        let map: SourceMap = sample_map();

        let value: serde_json::Value =
            serde_json::from_str(&map.to_json_string())
                .expect("serialized map parses");

        let parsed: SourceMap =
            parse_map_value(&value).expect("the map object parses");

        assert_eq!(parsed.to_json_string(), map.to_json_string());
    }
}
