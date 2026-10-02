use std::borrow::Cow;

use oxc::transformer::HelperLoaderMode as OxcHelperLoaderMode;
use oxc::transformer::HelperLoaderOptions as OxcHelperLoaderOptions;

/// The default helper module name, matching oxc's own default.
pub const DEFAULT_HELPER_MODULE_NAME: &str = "@oxc-project/runtime";

/// The helper loader mode.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum HelperLoaderMode {
    /// Helper functions are directly inserted into the program.
    #[default]
    Inline,
    /// Helper functions are accessed from a global `babelHelpers` object.
    External,
    /// Helper functions are imported from the runtime package.
    Runtime,
}

impl HelperLoaderMode {
    /// The mode fed to the oxc transformer; `Inline` maps to oxc `Runtime`,
    /// and the inline helpers pass replaces the emitted runtime imports.
    pub(crate) fn to_oxc_mode(self) -> OxcHelperLoaderMode {
        match self {
            | Self::Inline | Self::Runtime => OxcHelperLoaderMode::Runtime,
            | Self::External => OxcHelperLoaderMode::External,
        }
    }
}

/// The helper loader options.
#[derive(Debug, Clone, Default)]
pub struct HelperLoaderOptions {
    /// Strategy used to resolve helper calls.
    pub mode: Option<HelperLoaderMode>,
    /// The module name to import helper functions from, work in [HelperLoaderMode::Runtime].
    pub module_name: Option<String>,
}

/// The resolved helper-loader state consumed by the inline helpers pass.
#[derive(Debug, Clone)]
pub struct ResolvedHelpers {
    /// The resolved helper loader mode.
    pub mode: HelperLoaderMode,
    /// The module name helpers are imported from.
    pub module_name: String,
}

impl ResolvedHelpers {
    /// Whether the inline helpers pass must run.
    pub fn inline(&self) -> bool {
        matches!(self.mode, HelperLoaderMode::Inline)
    }

    /// The mode fed to the oxc transformer.
    pub fn oxc_mode(&self) -> OxcHelperLoaderMode {
        self.mode.to_oxc_mode()
    }
}

/// Resolve the effective helper state from the telarel layer.
///
/// The raw `oxc` base layer's helper state is ignored entirely; absent
/// telarel options default to [`HelperLoaderMode::default`] (`Inline`) and
/// [`DEFAULT_HELPER_MODULE_NAME`].
pub fn resolve_helpers(
    helper: Option<&HelperLoaderOptions>
) -> ResolvedHelpers {
    let mode: HelperLoaderMode =
        helper.and_then(|h| h.mode).unwrap_or_default();
    let module_name: String = helper
        .and_then(|h| h.module_name.clone())
        .unwrap_or_else(|| String::from(DEFAULT_HELPER_MODULE_NAME));

    ResolvedHelpers { mode, module_name }
}

/// Overlay the resolved helper state onto the oxc base; the module name is
/// kept so the inline helpers pass can find the imports it must replace.
///
/// The oxc base's previous helper state is overwritten, never read.
pub fn overlay_helper_loader(
    base: &mut OxcHelperLoaderOptions,
    helper: Option<&HelperLoaderOptions>,
) {
    let resolved: ResolvedHelpers = resolve_helpers(helper);

    base.mode = resolved.oxc_mode();
    base.module_name = Cow::Owned(resolved.module_name.clone());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn oxc_base() -> OxcHelperLoaderOptions {
        OxcHelperLoaderOptions::default()
    }

    #[test]
    fn test_default_mode_is_inline() {
        let mode: HelperLoaderMode = HelperLoaderMode::default();

        assert_eq!(mode, HelperLoaderMode::Inline);
    }

    #[test]
    fn test_to_oxc_mode_inline_maps_to_runtime() {
        let mode: OxcHelperLoaderMode = HelperLoaderMode::Inline.to_oxc_mode();

        assert!(matches!(mode, OxcHelperLoaderMode::Runtime));
    }

    #[test]
    fn test_to_oxc_mode_runtime_maps_to_runtime() {
        let mode: OxcHelperLoaderMode = HelperLoaderMode::Runtime.to_oxc_mode();

        assert!(matches!(mode, OxcHelperLoaderMode::Runtime));
    }

    #[test]
    fn test_to_oxc_mode_external_maps_to_external() {
        let mode: OxcHelperLoaderMode =
            HelperLoaderMode::External.to_oxc_mode();

        assert!(matches!(mode, OxcHelperLoaderMode::External));
    }

    #[test]
    fn test_resolve_helpers_none_defaults_to_inline() {
        let resolved: ResolvedHelpers = resolve_helpers(None);

        assert_eq!(resolved.mode, HelperLoaderMode::Inline);
        assert_eq!(resolved.module_name, DEFAULT_HELPER_MODULE_NAME);
        assert!(resolved.inline());
    }

    #[test]
    fn test_resolve_helpers_mode_none_defaults_to_inline() {
        let helper: HelperLoaderOptions =
            HelperLoaderOptions { mode: None, module_name: None };

        let resolved: ResolvedHelpers = resolve_helpers(Some(&helper));

        assert_eq!(resolved.mode, HelperLoaderMode::Inline);
        assert_eq!(resolved.module_name, DEFAULT_HELPER_MODULE_NAME);
        assert!(resolved.inline());
    }

    #[test]
    fn test_resolve_helpers_configured_inline() {
        let helper: HelperLoaderOptions = HelperLoaderOptions {
            mode: Some(HelperLoaderMode::Inline),
            module_name: None,
        };

        let resolved: ResolvedHelpers = resolve_helpers(Some(&helper));

        assert!(resolved.inline());
        assert_eq!(resolved.module_name, DEFAULT_HELPER_MODULE_NAME);
    }

    #[test]
    fn test_resolve_helpers_module_name_passthrough() {
        let helper: HelperLoaderOptions = HelperLoaderOptions {
            mode: Some(HelperLoaderMode::Inline),
            module_name: Some(String::from("my-runtime")),
        };

        let resolved: ResolvedHelpers = resolve_helpers(Some(&helper));

        assert!(resolved.inline());
        assert_eq!(resolved.module_name, "my-runtime");
    }

    #[test]
    fn test_overlay_helper_loader_inline_writes_oxc_runtime() {
        let helper: HelperLoaderOptions = HelperLoaderOptions {
            mode: Some(HelperLoaderMode::Inline),
            module_name: Some(String::from("my-runtime")),
        };

        let mut base: OxcHelperLoaderOptions = oxc_base();

        overlay_helper_loader(&mut base, Some(&helper));

        assert!(matches!(base.mode, OxcHelperLoaderMode::Runtime));
        assert_eq!(base.module_name, "my-runtime");
    }

    #[test]
    fn test_overlay_helper_loader_external_keeps_external() {
        let helper: HelperLoaderOptions = HelperLoaderOptions {
            mode: Some(HelperLoaderMode::External),
            module_name: None,
        };

        let mut base: OxcHelperLoaderOptions = oxc_base();

        overlay_helper_loader(&mut base, Some(&helper));

        assert!(matches!(base.mode, OxcHelperLoaderMode::External));
        assert_eq!(base.module_name, DEFAULT_HELPER_MODULE_NAME);
    }

    #[test]
    fn test_overlay_helper_loader_ignores_oxc_base_inline() {
        let mut base: OxcHelperLoaderOptions = OxcHelperLoaderOptions {
            mode: OxcHelperLoaderMode::Inline,
            module_name: Cow::Borrowed("my-runtime"),
        };

        overlay_helper_loader(&mut base, None);

        assert!(matches!(base.mode, OxcHelperLoaderMode::Runtime));
        assert_eq!(base.module_name, DEFAULT_HELPER_MODULE_NAME);
    }
}
