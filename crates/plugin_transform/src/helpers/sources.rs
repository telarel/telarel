/// An embedded helper: name and ESM source.
#[derive(Debug)]
pub struct HelperSource {
    /// The helper name; primary entries match the oxc `Helper` enum names.
    pub name: &'static str,
    /// The embedded ESM source from the installed `@oxc-project/runtime` package.
    pub esm_source: &'static str,
}

/// The oxc runtime helpers, mirroring its `Helper` enum exactly.
pub const HELPERS: &[HelperSource] = &[
    HelperSource {
        name: "awaitAsyncGenerator",
        esm_source: include_str!(concat!(
            env!("TELAREL_RUNTIME_HELPERS_DIR"),
            "/awaitAsyncGenerator.js"
        )),
    },
    HelperSource {
        name: "asyncGeneratorDelegate",
        esm_source: include_str!(concat!(
            env!("TELAREL_RUNTIME_HELPERS_DIR"),
            "/asyncGeneratorDelegate.js"
        )),
    },
    HelperSource {
        name: "asyncIterator",
        esm_source: include_str!(concat!(
            env!("TELAREL_RUNTIME_HELPERS_DIR"),
            "/asyncIterator.js"
        )),
    },
    HelperSource {
        name: "asyncToGenerator",
        esm_source: include_str!(concat!(
            env!("TELAREL_RUNTIME_HELPERS_DIR"),
            "/asyncToGenerator.js"
        )),
    },
    HelperSource {
        name: "objectSpread2",
        esm_source: include_str!(concat!(
            env!("TELAREL_RUNTIME_HELPERS_DIR"),
            "/objectSpread2.js"
        )),
    },
    HelperSource {
        name: "wrapAsyncGenerator",
        esm_source: include_str!(concat!(
            env!("TELAREL_RUNTIME_HELPERS_DIR"),
            "/wrapAsyncGenerator.js"
        )),
    },
    HelperSource {
        name: "extends",
        esm_source: include_str!(concat!(
            env!("TELAREL_RUNTIME_HELPERS_DIR"),
            "/extends.js"
        )),
    },
    HelperSource {
        name: "objectDestructuringEmpty",
        esm_source: include_str!(concat!(
            env!("TELAREL_RUNTIME_HELPERS_DIR"),
            "/objectDestructuringEmpty.js"
        )),
    },
    HelperSource {
        name: "objectWithoutProperties",
        esm_source: include_str!(concat!(
            env!("TELAREL_RUNTIME_HELPERS_DIR"),
            "/objectWithoutProperties.js"
        )),
    },
    HelperSource {
        name: "toPropertyKey",
        esm_source: include_str!(concat!(
            env!("TELAREL_RUNTIME_HELPERS_DIR"),
            "/toPropertyKey.js"
        )),
    },
    HelperSource {
        name: "defineProperty",
        esm_source: include_str!(concat!(
            env!("TELAREL_RUNTIME_HELPERS_DIR"),
            "/defineProperty.js"
        )),
    },
    HelperSource {
        name: "classPrivateFieldInitSpec",
        esm_source: include_str!(concat!(
            env!("TELAREL_RUNTIME_HELPERS_DIR"),
            "/classPrivateFieldInitSpec.js"
        )),
    },
    HelperSource {
        name: "classPrivateMethodInitSpec",
        esm_source: include_str!(concat!(
            env!("TELAREL_RUNTIME_HELPERS_DIR"),
            "/classPrivateMethodInitSpec.js"
        )),
    },
    HelperSource {
        name: "classPrivateFieldGet2",
        esm_source: include_str!(concat!(
            env!("TELAREL_RUNTIME_HELPERS_DIR"),
            "/classPrivateFieldGet2.js"
        )),
    },
    HelperSource {
        name: "classPrivateFieldSet2",
        esm_source: include_str!(concat!(
            env!("TELAREL_RUNTIME_HELPERS_DIR"),
            "/classPrivateFieldSet2.js"
        )),
    },
    HelperSource {
        name: "assertClassBrand",
        esm_source: include_str!(concat!(
            env!("TELAREL_RUNTIME_HELPERS_DIR"),
            "/assertClassBrand.js"
        )),
    },
    HelperSource {
        name: "toSetter",
        esm_source: include_str!(concat!(
            env!("TELAREL_RUNTIME_HELPERS_DIR"),
            "/toSetter.js"
        )),
    },
    HelperSource {
        name: "classPrivateFieldLooseKey",
        esm_source: include_str!(concat!(
            env!("TELAREL_RUNTIME_HELPERS_DIR"),
            "/classPrivateFieldLooseKey.js"
        )),
    },
    HelperSource {
        name: "classPrivateFieldLooseBase",
        esm_source: include_str!(concat!(
            env!("TELAREL_RUNTIME_HELPERS_DIR"),
            "/classPrivateFieldLooseBase.js"
        )),
    },
    HelperSource {
        name: "superPropGet",
        esm_source: include_str!(concat!(
            env!("TELAREL_RUNTIME_HELPERS_DIR"),
            "/superPropGet.js"
        )),
    },
    HelperSource {
        name: "superPropSet",
        esm_source: include_str!(concat!(
            env!("TELAREL_RUNTIME_HELPERS_DIR"),
            "/superPropSet.js"
        )),
    },
    HelperSource {
        name: "readOnlyError",
        esm_source: include_str!(concat!(
            env!("TELAREL_RUNTIME_HELPERS_DIR"),
            "/readOnlyError.js"
        )),
    },
    HelperSource {
        name: "writeOnlyError",
        esm_source: include_str!(concat!(
            env!("TELAREL_RUNTIME_HELPERS_DIR"),
            "/writeOnlyError.js"
        )),
    },
    HelperSource {
        name: "checkInRHS",
        esm_source: include_str!(concat!(
            env!("TELAREL_RUNTIME_HELPERS_DIR"),
            "/checkInRHS.js"
        )),
    },
    HelperSource {
        name: "decorate",
        esm_source: include_str!(concat!(
            env!("TELAREL_RUNTIME_HELPERS_DIR"),
            "/decorate.js"
        )),
    },
    HelperSource {
        name: "decorateParam",
        esm_source: include_str!(concat!(
            env!("TELAREL_RUNTIME_HELPERS_DIR"),
            "/decorateParam.js"
        )),
    },
    HelperSource {
        name: "decorateMetadata",
        esm_source: include_str!(concat!(
            env!("TELAREL_RUNTIME_HELPERS_DIR"),
            "/decorateMetadata.js"
        )),
    },
    HelperSource {
        name: "usingCtx",
        esm_source: include_str!(concat!(
            env!("TELAREL_RUNTIME_HELPERS_DIR"),
            "/usingCtx.js"
        )),
    },
    HelperSource {
        name: "taggedTemplateLiteral",
        esm_source: include_str!(concat!(
            env!("TELAREL_RUNTIME_HELPERS_DIR"),
            "/taggedTemplateLiteral.js"
        )),
    },
];

/// The helpers in the transitive dependency closure of [`HELPERS`] that are
/// not part of the oxc `Helper` enum; [`lookup`] covers both tables.
pub const DEPENDENCY_HELPERS: &[HelperSource] = &[
    HelperSource {
        name: "OverloadYield",
        esm_source: include_str!(concat!(
            env!("TELAREL_RUNTIME_HELPERS_DIR"),
            "/OverloadYield.js"
        )),
    },
    HelperSource {
        name: "checkPrivateRedeclaration",
        esm_source: include_str!(concat!(
            env!("TELAREL_RUNTIME_HELPERS_DIR"),
            "/checkPrivateRedeclaration.js"
        )),
    },
    HelperSource {
        name: "get",
        esm_source: include_str!(concat!(
            env!("TELAREL_RUNTIME_HELPERS_DIR"),
            "/get.js"
        )),
    },
    HelperSource {
        name: "getPrototypeOf",
        esm_source: include_str!(concat!(
            env!("TELAREL_RUNTIME_HELPERS_DIR"),
            "/getPrototypeOf.js"
        )),
    },
    HelperSource {
        name: "objectWithoutPropertiesLoose",
        esm_source: include_str!(concat!(
            env!("TELAREL_RUNTIME_HELPERS_DIR"),
            "/objectWithoutPropertiesLoose.js"
        )),
    },
    HelperSource {
        name: "set",
        esm_source: include_str!(concat!(
            env!("TELAREL_RUNTIME_HELPERS_DIR"),
            "/set.js"
        )),
    },
    HelperSource {
        name: "superPropBase",
        esm_source: include_str!(concat!(
            env!("TELAREL_RUNTIME_HELPERS_DIR"),
            "/superPropBase.js"
        )),
    },
    HelperSource {
        name: "toPrimitive",
        esm_source: include_str!(concat!(
            env!("TELAREL_RUNTIME_HELPERS_DIR"),
            "/toPrimitive.js"
        )),
    },
    HelperSource {
        name: "typeof",
        esm_source: include_str!(concat!(
            env!("TELAREL_RUNTIME_HELPERS_DIR"),
            "/typeof.js"
        )),
    },
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
                !helper.esm_source.is_empty(),
                "{} ESM source must be non-empty",
                helper.name
            );
        }
    }

    fn dependency_imports(helper: &HelperSource) -> Vec<&str> {
        helper
            .esm_source
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
