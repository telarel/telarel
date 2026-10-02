use std::collections::BTreeMap;

/// Shared cross-plugin ambient state for one compile run;
/// values must be JSON-serializable.
pub type PluginState = BTreeMap<String, serde_json::Value>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_state_maps_string_keys_to_json_values() {
        let mut state: PluginState = PluginState::default();

        state.insert(String::from("count"), serde_json::json!(7));
        state.insert(String::from("label"), serde_json::json!("a"));

        assert_eq!(state.get("count"), Some(&serde_json::json!(7)));
        assert_eq!(state.get("label"), Some(&serde_json::json!("a")));
        assert_eq!(state.get("missing"), None);
    }

    #[test]
    fn test_plugin_state_keys_are_ordered() {
        let mut state: PluginState = PluginState::default();

        state.insert(String::from("b"), serde_json::json!(2));
        state.insert(String::from("a"), serde_json::json!(1));

        let keys: Vec<&String> = state.keys().collect();

        assert_eq!(keys, vec![&String::from("a"), &String::from("b")]);
    }
}
