use oxc::transformer::TypeScriptOptions as OxcTypeScriptOptions;
use serde::Deserialize;

/// The TypeScript options.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TypeScriptOptions {
    #[serde(default)]
    pub only_remove_type_imports: Option<bool>,
    #[serde(default)]
    pub allow_namespaces: Option<bool>,
    #[serde(default)]
    pub jsx_pragma: Option<String>,
    #[serde(default)]
    pub jsx_pragma_frag: Option<String>,
    #[serde(default)]
    pub optimize_enums: Option<bool>,
}

/// Overlay `Some` TypeScript fields onto the oxc base, field-wise.
pub fn overlay_typescript(
    base: &mut OxcTypeScriptOptions,
    typescript: &TypeScriptOptions,
) {
    if let Some(only_remove_type_imports) = typescript.only_remove_type_imports
    {
        base.only_remove_type_imports = only_remove_type_imports;
    }

    if let Some(allow_namespaces) = typescript.allow_namespaces {
        base.allow_namespaces = allow_namespaces;
    }

    if let Some(jsx_pragma) = &typescript.jsx_pragma {
        base.jsx_pragma = jsx_pragma.clone().into();
    }

    if let Some(jsx_pragma_frag) = &typescript.jsx_pragma_frag {
        base.jsx_pragma_frag = jsx_pragma_frag.clone().into();
    }

    if let Some(optimize_enums) = typescript.optimize_enums {
        base.optimize_enums = optimize_enums;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_overlay_typescript_sets_only_given_fields() {
        let mut base: OxcTypeScriptOptions = OxcTypeScriptOptions::default();

        let typescript: TypeScriptOptions = TypeScriptOptions {
            only_remove_type_imports: Some(true),
            allow_namespaces: Some(false),
            optimize_enums: Some(true),
            ..TypeScriptOptions::default()
        };

        overlay_typescript(&mut base, &typescript);

        assert!(base.only_remove_type_imports);
        assert!(!base.allow_namespaces);
        assert!(base.optimize_enums);
        assert_eq!(base.jsx_pragma, "React.createElement");
    }

    #[test]
    fn test_overlay_typescript_pragmas() {
        let mut base: OxcTypeScriptOptions = OxcTypeScriptOptions::default();

        let typescript: TypeScriptOptions = TypeScriptOptions {
            jsx_pragma: Some(String::from("h")),
            jsx_pragma_frag: Some(String::from("Fragment")),
            ..TypeScriptOptions::default()
        };

        overlay_typescript(&mut base, &typescript);

        assert_eq!(base.jsx_pragma, "h");
        assert_eq!(base.jsx_pragma_frag, "Fragment");
    }
}
