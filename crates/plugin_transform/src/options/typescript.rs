use oxc::transformer::RewriteExtensionsMode;
use oxc::transformer::TransformOptions as OxcTransformOptions;
use serde::Deserialize;

/// Controls the TypeScript transform with tsconfig-compatible field names.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TypeScriptOptions {
    /// Rewrites relative import extensions in emitted code.
    ///
    /// By default, it is `false`.
    #[serde(default)]
    pub rewrite_relative_import_extensions: Option<bool>,
    /// Keeps value imports and exports, and removes only `type`-marked
    /// specifiers.
    ///
    /// By default, it is `false`.
    #[serde(default)]
    pub verbatim_module_syntax: Option<bool>,
    /// Emits decorator metadata. Requires `experimental_decorators`.
    ///
    /// By default, it is `false`.
    #[serde(default)]
    pub emit_decorator_metadata: Option<bool>,
    /// Lowers legacy TypeScript decorators.
    ///
    /// By default, it is `false`.
    #[serde(default)]
    pub experimental_decorators: Option<bool>,
    /// Uses define semantics for class fields when `true`.
    ///
    /// When `false`, the transform removes uninitialized fields and assigns
    /// initialized fields in the constructor.
    ///
    /// By default, it is `true`.
    #[serde(default)]
    pub use_define_for_class_fields: Option<bool>,
    /// Inlines const enum members and drops unused declarations.
    ///
    /// By default, it is `false`.
    #[serde(default)]
    pub optimize_const_enums: Option<bool>,
}

/// Resolve class-field semantics from the curated option, an explicit raw oxc
/// setting, or the default `true`, in that order.
fn overlay_class_fields(
    base: &mut OxcTransformOptions,
    typescript: Option<&TypeScriptOptions>,
) {
    if let Some(use_define_for_class_fields) =
        typescript.and_then(|typescript| typescript.use_define_for_class_fields)
    {
        base.assumptions.set_public_class_fields = !use_define_for_class_fields;

        base.typescript.remove_class_fields_without_initializer =
            !use_define_for_class_fields;

        base.typescript.allow_declare_fields = true;

        return;
    }

    let raw_configured: bool = base.assumptions.set_public_class_fields
        || base.typescript.remove_class_fields_without_initializer
        || !base.typescript.allow_declare_fields;

    if raw_configured {
        return;
    }

    base.assumptions.set_public_class_fields = false;
    base.typescript.remove_class_fields_without_initializer = false;
    base.typescript.allow_declare_fields = true;
}

/// Apply the TypeScript overlay to the oxc base.
///
/// The class-field semantics always resolve because the option owns its
/// default. The other fields apply only when set. Fields set to `None` keep
/// the base value.
pub fn overlay_typescript(
    base: &mut OxcTransformOptions,
    typescript: Option<&TypeScriptOptions>,
) {
    overlay_class_fields(base, typescript);

    let Some(typescript) = typescript else {
        return;
    };

    if let Some(rewrite_relative_import_extensions) =
        typescript.rewrite_relative_import_extensions
    {
        base.typescript.rewrite_import_extensions =
            rewrite_relative_import_extensions
                .then_some(RewriteExtensionsMode::Rewrite);
    }

    if let Some(verbatim_module_syntax) = typescript.verbatim_module_syntax {
        base.typescript.only_remove_type_imports = verbatim_module_syntax;
    }

    if let Some(emit_decorator_metadata) = typescript.emit_decorator_metadata {
        base.decorator.emit_decorator_metadata = emit_decorator_metadata;
    }

    if let Some(experimental_decorators) = typescript.experimental_decorators {
        base.decorator.legacy = experimental_decorators;
    }

    if let Some(optimize_const_enums) = typescript.optimize_const_enums {
        base.typescript.optimize_const_enums = optimize_const_enums;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn overlay(typescript: &TypeScriptOptions) -> OxcTransformOptions {
        let mut base: OxcTransformOptions = OxcTransformOptions::default();

        overlay_typescript(&mut base, Some(typescript));

        base
    }

    #[test]
    fn test_overlay_typescript_sets_only_given_fields() {
        let resolved: OxcTransformOptions = overlay(&TypeScriptOptions {
            verbatim_module_syntax: Some(true),
            optimize_const_enums: Some(true),
            ..TypeScriptOptions::default()
        });

        assert!(resolved.typescript.only_remove_type_imports);
        assert!(resolved.typescript.optimize_const_enums);
        assert!(!resolved.decorator.legacy);
        assert!(!resolved.decorator.emit_decorator_metadata);
        assert!(!resolved.assumptions.set_public_class_fields);
        assert!(!resolved.typescript.remove_class_fields_without_initializer);
        assert!(resolved.typescript.allow_declare_fields);
    }

    #[test]
    fn test_overlay_typescript_rewrite_relative_import_extensions() {
        let enabled: OxcTransformOptions = overlay(&TypeScriptOptions {
            rewrite_relative_import_extensions: Some(true),
            ..TypeScriptOptions::default()
        });

        assert!(matches!(
            enabled.typescript.rewrite_import_extensions,
            Some(RewriteExtensionsMode::Rewrite)
        ));

        let mut base: OxcTransformOptions = OxcTransformOptions::default();

        base.typescript.rewrite_import_extensions =
            Some(RewriteExtensionsMode::Rewrite);

        let disabled: TypeScriptOptions = TypeScriptOptions {
            rewrite_relative_import_extensions: Some(false),
            ..TypeScriptOptions::default()
        };

        overlay_typescript(&mut base, Some(&disabled));

        assert!(base.typescript.rewrite_import_extensions.is_none());
    }

    #[test]
    fn test_overlay_typescript_verbatim_module_syntax_clears_base() {
        let mut base: OxcTransformOptions = OxcTransformOptions::default();

        base.typescript.only_remove_type_imports = true;

        let typescript: TypeScriptOptions = TypeScriptOptions {
            verbatim_module_syntax: Some(false),
            ..TypeScriptOptions::default()
        };

        overlay_typescript(&mut base, Some(&typescript));

        assert!(!base.typescript.only_remove_type_imports);
    }

    #[test]
    fn test_overlay_typescript_decorators() {
        let enabled: OxcTransformOptions = overlay(&TypeScriptOptions {
            emit_decorator_metadata: Some(true),
            experimental_decorators: Some(true),
            ..TypeScriptOptions::default()
        });

        assert!(enabled.decorator.legacy);
        assert!(enabled.decorator.emit_decorator_metadata);

        let mut base: OxcTransformOptions = OxcTransformOptions::default();

        base.decorator.legacy = true;
        base.decorator.emit_decorator_metadata = true;

        let disabled: TypeScriptOptions = TypeScriptOptions {
            emit_decorator_metadata: Some(false),
            experimental_decorators: Some(false),
            ..TypeScriptOptions::default()
        };

        overlay_typescript(&mut base, Some(&disabled));

        assert!(!base.decorator.legacy);
        assert!(!base.decorator.emit_decorator_metadata);
    }

    #[test]
    fn test_overlay_typescript_use_define_for_class_fields() {
        let enabled: OxcTransformOptions = overlay(&TypeScriptOptions {
            use_define_for_class_fields: Some(true),
            ..TypeScriptOptions::default()
        });

        assert!(!enabled.assumptions.set_public_class_fields);
        assert!(!enabled.typescript.remove_class_fields_without_initializer);
        assert!(enabled.typescript.allow_declare_fields);

        let disabled: OxcTransformOptions = overlay(&TypeScriptOptions {
            use_define_for_class_fields: Some(false),
            ..TypeScriptOptions::default()
        });

        assert!(disabled.assumptions.set_public_class_fields);
        assert!(disabled.typescript.remove_class_fields_without_initializer);
        assert!(disabled.typescript.allow_declare_fields);
    }

    #[test]
    fn test_overlay_typescript_defaults_class_fields_to_define() {
        let mut base: OxcTransformOptions = OxcTransformOptions::default();

        overlay_typescript(&mut base, None);

        assert!(!base.assumptions.set_public_class_fields);
        assert!(!base.typescript.remove_class_fields_without_initializer);
        assert!(base.typescript.allow_declare_fields);
    }

    #[test]
    fn test_overlay_typescript_keeps_raw_class_fields_when_unset() {
        let mut base: OxcTransformOptions = OxcTransformOptions::default();

        base.assumptions.set_public_class_fields = true;
        base.typescript.remove_class_fields_without_initializer = false;
        base.typescript.allow_declare_fields = false;

        overlay_typescript(&mut base, None);

        assert!(base.assumptions.set_public_class_fields);
        assert!(!base.typescript.remove_class_fields_without_initializer);
        assert!(!base.typescript.allow_declare_fields);
    }

    #[test]
    fn test_overlay_typescript_explicit_class_fields_win_over_raw() {
        let mut base: OxcTransformOptions = OxcTransformOptions::default();

        base.assumptions.set_public_class_fields = true;
        base.typescript.remove_class_fields_without_initializer = true;
        base.typescript.allow_declare_fields = false;

        let typescript: TypeScriptOptions = TypeScriptOptions {
            use_define_for_class_fields: Some(true),
            ..TypeScriptOptions::default()
        };

        overlay_typescript(&mut base, Some(&typescript));

        assert!(!base.assumptions.set_public_class_fields);
        assert!(!base.typescript.remove_class_fields_without_initializer);
        assert!(base.typescript.allow_declare_fields);
    }

    #[test]
    fn test_overlay_typescript_keeps_base_when_unset() {
        let mut base: OxcTransformOptions = OxcTransformOptions::default();

        base.typescript.only_remove_type_imports = true;
        base.typescript.optimize_const_enums = true;
        base.typescript.rewrite_import_extensions =
            Some(RewriteExtensionsMode::Remove);
        base.typescript.remove_class_fields_without_initializer = true;
        base.assumptions.set_public_class_fields = true;
        base.decorator.legacy = true;
        base.decorator.emit_decorator_metadata = true;

        overlay_typescript(&mut base, Some(&TypeScriptOptions::default()));

        assert!(base.typescript.only_remove_type_imports);
        assert!(base.typescript.optimize_const_enums);
        assert!(matches!(
            base.typescript.rewrite_import_extensions,
            Some(RewriteExtensionsMode::Remove)
        ));
        assert!(base.typescript.remove_class_fields_without_initializer);
        assert!(base.assumptions.set_public_class_fields);
        assert!(base.decorator.legacy);
        assert!(base.decorator.emit_decorator_metadata);
    }
}
