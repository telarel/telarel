use std::collections::HashMap;
use std::collections::HashSet;

use anyhow::anyhow;
use oxc::allocator::Allocator;
use oxc::allocator::Vec as ArenaVec;
use oxc::ast::ast::ExportNamedDeclaration;
use oxc::ast::ast::ImportDeclarationSpecifier;
use oxc::ast::ast::ModuleExportName;
use oxc::ast::ast::Program;
use oxc::ast::ast::Statement;
use oxc::ast_visit::VisitMut;
use oxc::parser::Parser;
use oxc::semantic::Scoping;
use oxc::semantic::SemanticBuilder;
use oxc::span::SourceType;
use oxc::str::Str;
use oxc::syntax::symbol::SymbolId;

use crate::helpers::rewriter::Rewriter;
use crate::helpers::sources::HelperSource;
use crate::helpers::sources::lookup;

/// Whether the module export name is the default export (`default`).
fn is_default_export_name(name: &ModuleExportName<'_>) -> bool {
    match name {
        | ModuleExportName::IdentifierName(name) => name.name == "default",
        | ModuleExportName::StringLiteral(literal) => {
            literal.value == "default"
        },
        | ModuleExportName::IdentifierReference(_) => false,
    }
}

/// Extract the local name exported as `default` by `export { x as default };`.
fn default_export_local_name(body: &[Statement<'_>]) -> Option<String> {
    for statement in body {
        let Statement::ExportNamedDeclaration(export) = statement else {
            continue;
        };

        for specifier in &export.specifiers {
            let exported_is_default: bool =
                is_default_export_name(&specifier.exported);

            if !exported_is_default {
                continue;
            }

            if let ModuleExportName::IdentifierReference(local) =
                &specifier.local
            {
                return Some(local.name.as_str().to_string());
            }
        }
    }

    None
}

/// The vendored default-export statements are exactly `export { x as default };`
/// — a specifier-only named export re-exporting the default. Anything else is
/// a vendoring error and fails loudly.
fn validate_default_export_statement(
    export: &ExportNamedDeclaration<'_>
) -> anyhow::Result<()> {
    let mut default_specifiers: usize = 0;

    for specifier in &export.specifiers {
        if !is_default_export_name(&specifier.exported) {
            return Err(anyhow!(
                "vendored helper default export statement has a non-default specifier"
            ));
        }

        default_specifiers += 1;
    }

    if default_specifiers != 1 {
        return Err(anyhow!(
            "vendored helper default export statement must have exactly one specifier"
        ));
    }

    Ok(())
}

/// The helper inliner: parses vendored sources, resolves the dependency
/// closure, renames top-level bindings and accumulates the statements to
/// splice into the program.
pub struct Inliner<'a, 's> {
    pub allocator: &'a Allocator,
    /// Scoping of the transformed program; used to deconflict fresh uid names.
    pub scoping: &'s Scoping,
    /// Binding names claimed by this pass (primary uid names and fresh names).
    pub claimed: HashSet<String>,
    /// Final binding name per inlined helper name.
    pub resolved: HashMap<&'static str, String>,
    /// Inlined statements in emission order (dependencies before dependents).
    pub emitted: Vec<Statement<'a>>,
}

impl<'a, 's> Inliner<'a, 's> {
    /// Whether `name` is taken by any binding in the program, any unresolved
    /// (global) reference, or any name claimed by this pass — the same
    /// catalogue oxc's uid generator checks.
    fn name_taken(
        &self,
        name: &str,
    ) -> bool {
        self.claimed.contains(name)
            || self
                .scoping
                .symbol_names()
                .any(|symbol_name| symbol_name == name)
            || self
                .scoping
                .root_unresolved_references()
                .keys()
                .any(|key| key.as_str() == name)
    }

    /// Generate a uid name that does not clash with any identifier in the
    /// program or any name claimed by this pass, following oxc's uid
    /// convention (`_base`, then `_base2`, `_base3`, ...).
    fn fresh_name(
        &self,
        base: &str,
    ) -> String {
        let mut count: u32 = 1;

        let mut name: String = base.to_string();

        while self.name_taken(&name) {
            count += 1;
            name = format!("{base}{count}");
        }

        name
    }

    /// Inline `helper` so its default export binds as `target_name`.
    ///
    /// Idempotent per helper: the first call fixes the binding name; later
    /// calls (from other helpers' dependency closures) are no-ops.
    pub fn ensure_inlined(
        &mut self,
        helper: &'static HelperSource,
        target_name: &str,
    ) -> anyhow::Result<()> {
        if self.resolved.contains_key(helper.name) {
            return Ok(());
        }

        // Insert before recursing so a (non-existing today) dependency cycle
        // cannot recurse infinitely.
        self.resolved.insert(helper.name, target_name.to_string());
        self.claimed.insert(target_name.to_string());

        let parsed: oxc::parser::ParserReturn<'a> =
            Parser::new(self.allocator, helper.esm_source, SourceType::mjs())
                .parse();

        if parsed.fatal_error || parsed.diagnostics.has_errors() {
            return Err(anyhow!(
                "vendored helper `{}` does not parse",
                helper.name
            ));
        }

        let mut helper_program: Program<'a> = parsed.program;

        let helper_scoping: Scoping = SemanticBuilder::new()
            .build(&helper_program)
            .semantic
            .into_scoping();

        let default_local_name: String = default_export_local_name(
            &helper_program.body,
        )
        .ok_or_else(|| {
            anyhow!(
                "vendored helper `{}` has no `export {{ x as default }}`",
                helper.name
            )
        })?;

        // Drop the dependency imports first (each dependency is inlined
        // separately); the default-export statement is validated and dropped,
        // and every other statement is kept.
        //
        // Import locals are root bindings too, so this scan must run before
        // the top-level rename pass below: their bindings are renamed via the
        // dependency handling (to the dependency's final name), never via
        // fresh uid names.
        let helper_body: ArenaVec<'a, Statement<'a>> = std::mem::replace(
            &mut helper_program.body,
            ArenaVec::new_in(&self.allocator),
        );

        let mut kept: ArenaVec<'a, Statement<'a>> =
            ArenaVec::new_in(&self.allocator);

        let mut import_symbol_ids: HashSet<SymbolId> = HashSet::new();

        let mut renames: HashMap<SymbolId, String> = HashMap::new();

        for statement in helper_body {
            match statement {
                | Statement::ImportDeclaration(import) => {
                    self.inline_dependency(
                        helper,
                        &import.source.value,
                        &import.specifiers,
                        &mut import_symbol_ids,
                        &mut renames,
                    )?;
                },
                | Statement::ExportNamedDeclaration(export) => {
                    validate_default_export_statement(&export)?;
                },
                | Statement::ExportFromDeclaration(_)
                | Statement::ExportDefaultDeclaration(_)
                | Statement::ExportAllDeclaration(_) => {
                    return Err(anyhow!(
                        "vendored helper `{}` has an unexpected export statement",
                        helper.name
                    ));
                },
                | statement => {
                    kept.push(statement);
                },
            }
        }

        // Rename every remaining top-level binding of the helper: the default
        // export to `target_name`, all others (helper-internal functions and
        // vars, e.g. `var id = 0` in classPrivateFieldLooseKey) to fresh uid
        // names.
        let mut renamed_default: bool = false;

        for (name, symbol_id) in
            helper_scoping.get_bindings(helper_scoping.root_scope_id()).iter()
        {
            let name: &str = name.as_str();

            if import_symbol_ids.contains(symbol_id) {
                continue;
            }

            let final_name: String = if name == default_local_name {
                renamed_default = true;

                target_name.to_string()
            } else {
                self.fresh_name(&format!("_{name}"))
            };

            self.claimed.insert(final_name.clone());

            renames.insert(*symbol_id, final_name);
        }

        if !renamed_default {
            return Err(anyhow!(
                "vendored helper `{}` default export `{default_local_name}` is not a top-level binding",
                helper.name
            ));
        }

        let mut rewriter: Rewriter<'a, '_> = Rewriter {
            allocator: self.allocator,
            scoping: &helper_scoping,
            renames,
            reset_spans: true,
        };

        for statement in &mut kept {
            rewriter.visit_statement(statement);
        }

        self.emitted.extend(kept);

        Ok(())
    }

    /// Resolve one relative dependency import of `helper` (`./<name>.js`),
    /// inlining the dependency under a fresh uid name and recording the rename
    /// from the import's local binding to the final dependency name.
    /// Mutually recursive with [`Self::ensure_inlined`]; `resolved` is filled
    /// before recursing, so dependency cycles cannot loop forever.
    fn inline_dependency(
        &mut self,
        helper: &HelperSource,
        source: &Str<'a>,
        specifiers: &Option<ArenaVec<'a, ImportDeclarationSpecifier<'a>>>,
        import_symbol_ids: &mut HashSet<SymbolId>,
        renames: &mut HashMap<SymbolId, String>,
    ) -> anyhow::Result<()> {
        let source_value: &str = source.as_str();

        let Some(dep_name) = source_value
            .strip_prefix("./")
            .and_then(|rest| rest.strip_suffix(".js"))
        else {
            return Err(anyhow!(
                "vendored helper `{}` has a non-relative import `{source_value}`",
                helper.name
            ));
        };

        let Some(dep_helper) = lookup(dep_name) else {
            return Err(anyhow!(
                "vendored helper `{}` depends on unknown helper `{dep_name}`",
                helper.name
            ));
        };

        let Some(specifiers) = specifiers else {
            return Err(anyhow!(
                "vendored helper `{}` dependency `{dep_name}` has no import specifiers",
                helper.name
            ));
        };

        let [ImportDeclarationSpecifier::ImportDefaultSpecifier(default)] =
            specifiers.as_slice()
        else {
            return Err(anyhow!(
                "vendored helper `{}` dependency `{dep_name}` import is not a single default import",
                helper.name
            ));
        };

        let Some(local_symbol_id) = default.local.symbol_id.get() else {
            return Err(anyhow!(
                "vendored helper `{}` dependency `{dep_name}` import binding has no symbol",
                helper.name
            ));
        };

        let dep_final_name: String = match self.resolved.get(dep_helper.name) {
            | Some(final_name) => final_name.clone(),
            | None => {
                let fresh_name: String =
                    self.fresh_name(&format!("_{}", dep_helper.name));

                self.ensure_inlined(dep_helper, &fresh_name)?;

                fresh_name
            },
        };

        renames.insert(local_symbol_id, dep_final_name.clone());

        import_symbol_ids.insert(local_symbol_id);

        Ok(())
    }
}
