/// An embedded helper: name and source.
#[derive(Debug)]
pub struct HelperSource {
    /// The helper name; primary entries match the oxc `Helper` enum names.
    pub name: &'static str,
    /// The embedded source vendored from `@oxc-project/runtime`.
    pub source: &'static str,
}

/// The oxc runtime helpers, mirroring its `Helper` enum exactly.
pub const HELPERS: &[HelperSource] = &[
    HelperSource {
        name: "awaitAsyncGenerator",
        source: include_str!("runtime/awaitAsyncGenerator.js"),
    },
    HelperSource {
        name: "asyncGeneratorDelegate",
        source: include_str!("runtime/asyncGeneratorDelegate.js"),
    },
    HelperSource {
        name: "asyncIterator",
        source: include_str!("runtime/asyncIterator.js"),
    },
    HelperSource {
        name: "asyncToGenerator",
        source: include_str!("runtime/asyncToGenerator.js"),
    },
    HelperSource {
        name: "objectSpread2",
        source: include_str!("runtime/objectSpread2.js"),
    },
    HelperSource {
        name: "wrapAsyncGenerator",
        source: include_str!("runtime/wrapAsyncGenerator.js"),
    },
    HelperSource {
        name: "extends",
        source: include_str!("runtime/extends.js"),
    },
    HelperSource {
        name: "objectDestructuringEmpty",
        source: include_str!("runtime/objectDestructuringEmpty.js"),
    },
    HelperSource {
        name: "objectWithoutProperties",
        source: include_str!("runtime/objectWithoutProperties.js"),
    },
    HelperSource {
        name: "toPropertyKey",
        source: include_str!("runtime/toPropertyKey.js"),
    },
    HelperSource {
        name: "defineProperty",
        source: include_str!("runtime/defineProperty.js"),
    },
    HelperSource {
        name: "classPrivateFieldInitSpec",
        source: include_str!("runtime/classPrivateFieldInitSpec.js"),
    },
    HelperSource {
        name: "classPrivateMethodInitSpec",
        source: include_str!("runtime/classPrivateMethodInitSpec.js"),
    },
    HelperSource {
        name: "classPrivateFieldGet2",
        source: include_str!("runtime/classPrivateFieldGet2.js"),
    },
    HelperSource {
        name: "classPrivateFieldSet2",
        source: include_str!("runtime/classPrivateFieldSet2.js"),
    },
    HelperSource {
        name: "assertClassBrand",
        source: include_str!("runtime/assertClassBrand.js"),
    },
    HelperSource {
        name: "toSetter",
        source: include_str!("runtime/toSetter.js"),
    },
    HelperSource {
        name: "classPrivateFieldLooseKey",
        source: include_str!("runtime/classPrivateFieldLooseKey.js"),
    },
    HelperSource {
        name: "classPrivateFieldLooseBase",
        source: include_str!("runtime/classPrivateFieldLooseBase.js"),
    },
    HelperSource {
        name: "superPropGet",
        source: include_str!("runtime/superPropGet.js"),
    },
    HelperSource {
        name: "superPropSet",
        source: include_str!("runtime/superPropSet.js"),
    },
    HelperSource {
        name: "readOnlyError",
        source: include_str!("runtime/readOnlyError.js"),
    },
    HelperSource {
        name: "writeOnlyError",
        source: include_str!("runtime/writeOnlyError.js"),
    },
    HelperSource {
        name: "checkInRHS",
        source: include_str!("runtime/checkInRHS.js"),
    },
    HelperSource {
        name: "decorate",
        source: include_str!("runtime/decorate.js"),
    },
    HelperSource {
        name: "decorateParam",
        source: include_str!("runtime/decorateParam.js"),
    },
    HelperSource {
        name: "decorateMetadata",
        source: include_str!("runtime/decorateMetadata.js"),
    },
    HelperSource {
        name: "usingCtx",
        source: include_str!("runtime/usingCtx.js"),
    },
    HelperSource {
        name: "taggedTemplateLiteral",
        source: include_str!("runtime/taggedTemplateLiteral.js"),
    },
];

/// The helpers in the transitive dependency closure of [`HELPERS`] that are
/// not part of the oxc `Helper` enum; [`lookup`] covers both tables.
pub const DEPENDENCY_HELPERS: &[HelperSource] = &[
    HelperSource {
        name: "OverloadYield",
        source: include_str!("runtime/OverloadYield.js"),
    },
    HelperSource {
        name: "checkPrivateRedeclaration",
        source: include_str!("runtime/checkPrivateRedeclaration.js"),
    },
    HelperSource { name: "get", source: include_str!("runtime/get.js") },
    HelperSource {
        name: "getPrototypeOf",
        source: include_str!("runtime/getPrototypeOf.js"),
    },
    HelperSource {
        name: "objectWithoutPropertiesLoose",
        source: include_str!("runtime/objectWithoutPropertiesLoose.js"),
    },
    HelperSource { name: "set", source: include_str!("runtime/set.js") },
    HelperSource {
        name: "superPropBase",
        source: include_str!("runtime/superPropBase.js"),
    },
    HelperSource {
        name: "toPrimitive",
        source: include_str!("runtime/toPrimitive.js"),
    },
    HelperSource { name: "typeof", source: include_str!("runtime/typeof.js") },
];

/// Look up a helper by name, covering both the oxc helpers and the
/// dependency-only helpers.
pub fn lookup(name: &str) -> Option<&'static HelperSource> {
    let helper: Option<&'static HelperSource> = HELPERS
        .iter()
        .chain(DEPENDENCY_HELPERS.iter())
        .find(|helper: &&HelperSource| helper.name == name);

    helper
}

#[cfg(test)]
mod tests {
    use super::*;

    const OXC_HELPER_NAMES: [&str; 29] = [
        "awaitAsyncGenerator",
        "asyncGeneratorDelegate",
        "asyncIterator",
        "asyncToGenerator",
        "objectSpread2",
        "wrapAsyncGenerator",
        "extends",
        "objectDestructuringEmpty",
        "objectWithoutProperties",
        "toPropertyKey",
        "defineProperty",
        "classPrivateFieldInitSpec",
        "classPrivateMethodInitSpec",
        "classPrivateFieldGet2",
        "classPrivateFieldSet2",
        "assertClassBrand",
        "toSetter",
        "classPrivateFieldLooseKey",
        "classPrivateFieldLooseBase",
        "superPropGet",
        "superPropSet",
        "readOnlyError",
        "writeOnlyError",
        "checkInRHS",
        "decorate",
        "decorateParam",
        "decorateMetadata",
        "usingCtx",
        "taggedTemplateLiteral",
    ];

    #[test]
    fn test_helper_index_matches_oxc_enum() {
        let names: Vec<&str> =
            HELPERS.iter().map(|helper: &HelperSource| helper.name).collect();

        assert_eq!(names, OXC_HELPER_NAMES.to_vec());
    }

    #[test]
    fn test_lookup_resolves_every_helper_with_non_empty_source() {
        for helper in HELPERS.iter().chain(DEPENDENCY_HELPERS.iter()) {
            let helper: &HelperSource = helper;

            let resolved: Option<&'static HelperSource> = lookup(helper.name);

            assert!(resolved.is_some(), "helper {} must resolve", helper.name);

            assert!(
                !helper.source.is_empty(),
                "{} source must be non-empty",
                helper.name
            );
        }
    }

    fn dependency_imports(helper: &HelperSource) -> Vec<&str> {
        helper
            .source
            .lines()
            .filter_map(|line: &str| {
                let import: &str = line.strip_prefix("import ")?;
                let start: usize = import.find('"')? + 1;
                let end: usize = import.rfind('"')?;

                Some(&import[start..end])
            })
            .collect()
    }

    #[test]
    fn test_every_dependency_resolves() {
        for helper in DEPENDENCY_HELPERS.iter() {
            let helper: &HelperSource = helper;

            let resolved: Option<&'static HelperSource> = lookup(helper.name);

            assert!(
                resolved.is_some(),
                "dependency helper {} must resolve",
                helper.name
            );
        }
    }

    #[test]
    fn test_dependency_imports_use_relative_specifiers() {
        for helper in DEPENDENCY_HELPERS.iter() {
            let helper: &HelperSource = helper;

            for specifier in dependency_imports(helper) {
                assert!(
                    specifier.starts_with("./"),
                    "{} import specifier {} must be relative",
                    helper.name,
                    specifier
                );
            }
        }
    }

    #[test]
    fn test_lookup_unknown_name_returns_none() {
        assert!(lookup("doesNotExist").is_none());
    }
}
