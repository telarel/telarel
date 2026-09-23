use oxc::transformer::JsxOptions as OxcJsxOptions;
use oxc::transformer::JsxRuntime as OxcJsxRuntime;
use oxc::transformer::ReactRefreshOptions as OxcReactRefreshOptions;
use serde::Deserialize;

/// Which JSX runtime to emit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum JsxRuntime {
    Classic,
    Automatic,
}

/// The JSX options.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct JsxOptions {
    pub runtime: Option<JsxRuntime>,
    #[serde(default)]
    pub import_source: Option<String>,
    #[serde(default)]
    pub pragma: Option<String>,
    #[serde(default)]
    pub pragma_frag: Option<String>,
    #[serde(default)]
    pub development: Option<bool>,
    #[serde(default)]
    pub refresh: Option<bool>,
}

/// Overlay `Some` JSX fields onto the oxc base, field-wise.
pub fn overlay_jsx(
    base: &mut OxcJsxOptions,
    jsx: &JsxOptions,
) {
    if let Some(runtime) = jsx.runtime {
        base.runtime = match runtime {
            | JsxRuntime::Classic => OxcJsxRuntime::Classic,
            | JsxRuntime::Automatic => OxcJsxRuntime::Automatic,
        };
    }

    if let Some(import_source) = &jsx.import_source {
        base.import_source = Some(import_source.clone());
    }

    if let Some(pragma) = &jsx.pragma {
        base.pragma = Some(pragma.clone());
    }

    if let Some(pragma_frag) = &jsx.pragma_frag {
        base.pragma_frag = Some(pragma_frag.clone());
    }

    if let Some(development) = jsx.development {
        base.development = development;
    }

    if let Some(refresh) = jsx.refresh {
        base.refresh = refresh.then(OxcReactRefreshOptions::default);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_overlay_jsx_sets_only_given_fields() {
        let mut base: OxcJsxOptions = OxcJsxOptions::default();

        let jsx: JsxOptions = JsxOptions {
            runtime: Some(JsxRuntime::Automatic),
            import_source: Some(String::from("preact")),
            ..JsxOptions::default()
        };

        overlay_jsx(&mut base, &jsx);

        assert_eq!(base.runtime, OxcJsxRuntime::Automatic);
        assert_eq!(base.import_source.as_deref(), Some("preact"));
        assert_eq!(base.pragma, None);
    }

    #[test]
    fn test_overlay_jsx_classic_runtime() {
        let mut base: OxcJsxOptions = OxcJsxOptions::default();

        let jsx: JsxOptions = JsxOptions {
            runtime: Some(JsxRuntime::Classic),
            ..JsxOptions::default()
        };

        overlay_jsx(&mut base, &jsx);

        assert_eq!(base.runtime, OxcJsxRuntime::Classic);
    }

    #[test]
    fn test_overlay_jsx_refresh_enables_refresh_options() {
        let mut base: OxcJsxOptions = OxcJsxOptions::default();

        let jsx: JsxOptions =
            JsxOptions { refresh: Some(true), ..JsxOptions::default() };

        overlay_jsx(&mut base, &jsx);

        assert!(base.refresh.is_some());
    }

    #[test]
    fn test_overlay_jsx_refresh_false_clears_refresh() {
        let mut base: OxcJsxOptions = OxcJsxOptions {
            refresh: Some(OxcReactRefreshOptions::default()),
            ..OxcJsxOptions::default()
        };

        let jsx: JsxOptions =
            JsxOptions { refresh: Some(false), ..JsxOptions::default() };

        overlay_jsx(&mut base, &jsx);

        assert!(base.refresh.is_none());
    }
}
