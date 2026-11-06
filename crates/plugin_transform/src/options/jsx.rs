use oxc::transformer::JsxOptions as OxcJsxOptions;
use oxc::transformer::JsxRuntime as OxcJsxRuntime;
use oxc::transformer::ReactRefreshOptions as OxcReactRefreshOptions;
use serde::Deserialize;

/// How the transform handles JSX.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum JsxRuntime {
    /// Keeps JSX unchanged.
    #[default]
    Preserve,
    /// Lowers JSX with the classic `React.createElement` runtime.
    Classic,
    /// Lowers JSX with the automatic runtime.
    Automatic,
}

/// Controls the JSX transform.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct JsxOptions {
    /// Selects the JSX runtime. Defaults to `preserve`.
    pub runtime: Option<JsxRuntime>,
    /// Module imported for automatic-runtime helpers. Defaults to `react`.
    #[serde(default)]
    pub import_source: Option<String>,
    /// Function used for JSX elements by the classic runtime. Defaults to `React.createElement`.
    #[serde(default)]
    pub pragma: Option<String>,
    /// Component used for JSX fragments by the classic runtime. Defaults to `React.Fragment`.
    #[serde(default)]
    pub pragma_frag: Option<String>,
    /// Adds development-only `__source` and `__self` metadata to JSX elements.
    #[serde(default)]
    pub development: Option<bool>,
    /// Enables React Fast Refresh instrumentation.
    #[serde(default)]
    pub refresh: Option<bool>,
}

/// The runtime the JSX layer applies. `None` means [`JsxRuntime::Preserve`].
pub fn effective_runtime(jsx: Option<&JsxOptions>) -> JsxRuntime {
    jsx.and_then(|jsx| jsx.runtime).unwrap_or_default()
}

/// Apply the JSX layer to the oxc base. `None` means [`JsxRuntime::Preserve`].
///
/// Oxc has no preserve runtime, so `Preserve` clears every pass that would
/// rewrite JSX and leaves the remaining oxc fields untouched.
pub fn overlay_jsx(
    base: &mut OxcJsxOptions,
    jsx: Option<&JsxOptions>,
) {
    let runtime: JsxRuntime = effective_runtime(jsx);

    if matches!(runtime, JsxRuntime::Preserve) {
        base.jsx_plugin = false;
        base.display_name_plugin = false;
        base.jsx_self_plugin = false;
        base.jsx_source_plugin = false;
        base.development = false;
        base.refresh = None;

        return;
    }

    // A lowering runtime always carries its JSX options.
    let Some(jsx): Option<&JsxOptions> = jsx else {
        return;
    };

    base.runtime = if matches!(runtime, JsxRuntime::Classic) {
        OxcJsxRuntime::Classic
    } else {
        OxcJsxRuntime::Automatic
    };

    base.jsx_plugin = true;

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

    fn assert_preserved(base: &OxcJsxOptions) {
        assert!(!base.jsx_plugin);
        assert!(!base.display_name_plugin);
        assert!(!base.jsx_self_plugin);
        assert!(!base.jsx_source_plugin);
        assert!(!base.development);
        assert!(base.refresh.is_none());
    }

    #[test]
    fn test_jsx_runtime_default_is_preserve() {
        assert_eq!(JsxRuntime::default(), JsxRuntime::Preserve);
    }

    #[test]
    fn test_effective_runtime_none_preserves() {
        assert_eq!(effective_runtime(None), JsxRuntime::Preserve);
    }

    #[test]
    fn test_effective_runtime_reads_the_configured_runtime() {
        for runtime in
            [JsxRuntime::Preserve, JsxRuntime::Classic, JsxRuntime::Automatic]
        {
            let jsx: JsxOptions =
                JsxOptions { runtime: Some(runtime), ..JsxOptions::default() };

            assert_eq!(effective_runtime(Some(&jsx)), runtime);
        }
    }

    #[test]
    fn test_overlay_jsx_sets_only_given_fields() {
        let mut base: OxcJsxOptions = OxcJsxOptions::default();

        let jsx: JsxOptions = JsxOptions {
            runtime: Some(JsxRuntime::Automatic),
            import_source: Some(String::from("preact")),
            ..JsxOptions::default()
        };

        overlay_jsx(&mut base, Some(&jsx));

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

        overlay_jsx(&mut base, Some(&jsx));

        assert_eq!(base.runtime, OxcJsxRuntime::Classic);
    }

    #[test]
    fn test_overlay_jsx_lowering_forces_jsx_plugin() {
        for runtime in [JsxRuntime::Classic, JsxRuntime::Automatic] {
            let mut base: OxcJsxOptions = OxcJsxOptions::disable();

            let jsx: JsxOptions =
                JsxOptions { runtime: Some(runtime), ..JsxOptions::default() };

            overlay_jsx(&mut base, Some(&jsx));

            assert!(base.jsx_plugin, "{runtime:?}");
        }
    }

    #[test]
    fn test_overlay_jsx_none_preserves() {
        let mut base: OxcJsxOptions = OxcJsxOptions::default();

        overlay_jsx(&mut base, None);

        assert_preserved(&base);
    }

    #[test]
    fn test_overlay_jsx_preserve_clears_every_pass_flag() {
        let mut base: OxcJsxOptions = OxcJsxOptions {
            jsx_self_plugin: true,
            jsx_source_plugin: true,
            development: true,
            refresh: Some(OxcReactRefreshOptions::default()),
            ..OxcJsxOptions::default()
        };

        let jsx: JsxOptions = JsxOptions {
            runtime: Some(JsxRuntime::Preserve),
            import_source: Some(String::from("preact")),
            pragma: Some(String::from("h")),
            pragma_frag: Some(String::from("Fragment")),
            development: Some(true),
            refresh: Some(true),
        };

        overlay_jsx(&mut base, Some(&jsx));

        assert_preserved(&base);
        assert_eq!(base.import_source, None);
        assert_eq!(base.pragma, None);
        assert_eq!(base.pragma_frag, None);
    }

    #[test]
    fn test_overlay_jsx_refresh_enables_refresh_options() {
        let mut base: OxcJsxOptions = OxcJsxOptions::default();

        let jsx: JsxOptions = JsxOptions {
            runtime: Some(JsxRuntime::Automatic),
            refresh: Some(true),
            ..JsxOptions::default()
        };

        overlay_jsx(&mut base, Some(&jsx));

        assert!(base.refresh.is_some());
    }

    #[test]
    fn test_overlay_jsx_refresh_false_clears_refresh() {
        let mut base: OxcJsxOptions = OxcJsxOptions {
            refresh: Some(OxcReactRefreshOptions::default()),
            ..OxcJsxOptions::default()
        };

        let jsx: JsxOptions = JsxOptions {
            runtime: Some(JsxRuntime::Automatic),
            refresh: Some(false),
            ..JsxOptions::default()
        };

        overlay_jsx(&mut base, Some(&jsx));

        assert!(base.refresh.is_none());
    }
}
