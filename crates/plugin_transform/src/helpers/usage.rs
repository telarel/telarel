use oxc::ast::ast::Argument;
use oxc::ast::ast::BindingPattern;
use oxc::ast::ast::Expression;
use oxc::ast::ast::ImportDeclaration;
use oxc::ast::ast::ImportDeclarationSpecifier;
use oxc::ast::ast::ImportOrExportKind;
use oxc::ast::ast::Program;
use oxc::ast::ast::Statement;
use oxc::ast::ast::VariableDeclaration;
use oxc::ast::ast::VariableDeclarationKind;
use oxc::syntax::symbol::SymbolId;

use crate::helpers::sources::HelperSource;
use crate::helpers::sources::lookup;

/// A helper usage statement found in the transformed program.
pub struct HelperUsage {
    /// Index of the usage statement in `program.body`.
    pub statement_index: usize,
    /// The helper to inline.
    pub helper: &'static HelperSource,
    /// The local binding name oxc generated for the helper (a uid).
    pub binding_name: String,
    /// The symbol of the local binding, when known; used to rename the
    /// references of extra usages of an already-inlined helper.
    pub binding_symbol_id: Option<SymbolId>,
}

/// Resolve a `<module>/helpers/<name>` specifier to a vendored helper.
///
/// Returns `None` for specifiers that are not helper imports of the resolved
/// runtime module, including paths of helpers missing from the vendored index
/// (which can only come from user code).
fn helper_from_source(
    source_value: &str,
    module_name: &str,
) -> Option<&'static HelperSource> {
    let prefix: String = format!("{module_name}/helpers/");

    let helper_name: &str = source_value.strip_prefix(prefix.as_str())?;

    lookup(helper_name)
}

/// Match the exact oxc `import` form for one helper source.
///
/// Every specifier of the declaration must be a default import, since the
/// oxc helper loader only ever adds default imports; all the default locals
/// are returned as usages (oxc merges same-source imports into one
/// declaration, so a user default import from the same helper path can share
/// the declaration with an oxc-generated uid).
fn import_usage(
    import: &ImportDeclaration<'_>,
    module_name: &str,
    statement_index: usize,
) -> Option<Vec<HelperUsage>> {
    if import.import_kind != ImportOrExportKind::Value {
        return None;
    }

    let helper: &'static HelperSource =
        helper_from_source(import.source.value.as_str(), module_name)?;

    let specifiers: &[ImportDeclarationSpecifier<'_>] =
        import.specifiers.as_ref()?;

    if specifiers.is_empty() {
        return None;
    }

    let mut usages: Vec<HelperUsage> = Vec::new();

    for specifier in specifiers {
        let ImportDeclarationSpecifier::ImportDefaultSpecifier(default) =
            specifier
        else {
            return None;
        };

        usages.push(HelperUsage {
            statement_index,
            helper,
            binding_name: default.local.name.as_str().to_string(),
            binding_symbol_id: default.local.symbol_id.get(),
        });
    }

    Some(usages)
}

/// Match the exact oxc `require` form: a single-declarator `var` whose init is
/// `require("<module>/helpers/<name>")`.
fn require_usage(
    declaration: &VariableDeclaration<'_>,
    module_name: &str,
    statement_index: usize,
) -> Option<HelperUsage> {
    if declaration.kind != VariableDeclarationKind::Var {
        return None;
    }

    let [declarator] = declaration.declarations.as_slice() else {
        return None;
    };

    let BindingPattern::BindingIdentifier(binding) = &declarator.id else {
        return None;
    };

    let Some(init) = &declarator.init else {
        return None;
    };

    let Expression::CallExpression(call) = init else {
        return None;
    };

    let Expression::Identifier(callee) = &call.callee else {
        return None;
    };

    if callee.name != "require" {
        return None;
    }

    let [argument] = call.arguments.as_slice() else {
        return None;
    };

    let Argument::StringLiteral(source) = argument else {
        return None;
    };

    let helper: &'static HelperSource =
        helper_from_source(source.value.as_str(), module_name)?;

    Some(HelperUsage {
        statement_index,
        helper,
        binding_name: binding.name.as_str().to_string(),
        binding_symbol_id: binding.symbol_id.get(),
    })
}

/// Collect the helper usage statements (module `import` form and script
/// `require` form) in program body order.
///
/// Both oxc-emitted shapes are exact:
///
/// * `import <uid> from "<module>/helpers/<name>";` — one declaration per
///   source, default-import specifier(s) only, `ImportOrExportKind::Value`.
/// * `var <uid> = require("<module>/helpers/<name>");` — one `var`
///   declarator whose init is a `require(...)` call with one string literal.
///
/// Declarations that do not match the exact shape (for example user code that
/// imports from the runtime helpers path with named specifiers, or type-only
/// imports, which oxc never emits for helpers) are left untouched.
pub fn collect_usages(
    program: &Program<'_>,
    module_name: &str,
) -> Vec<HelperUsage> {
    let mut usages: Vec<HelperUsage> = Vec::new();

    for (statement_index, statement) in program.body.iter().enumerate() {
        match statement {
            | Statement::ImportDeclaration(import) => {
                let Some(usage) =
                    import_usage(import, module_name, statement_index)
                else {
                    continue;
                };

                usages.extend(usage);
            },
            | Statement::VariableDeclaration(declaration) => {
                let Some(usage) =
                    require_usage(declaration, module_name, statement_index)
                else {
                    continue;
                };

                usages.push(usage);
            },
            | _ => {},
        }
    }

    usages
}
