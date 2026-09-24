mod inliner;
mod rewriter;
mod sources;
mod usage;

use std::collections::HashMap;
use std::collections::HashSet;

use anyhow::anyhow;
use oxc::allocator::Allocator;
use oxc::allocator::Vec as ArenaVec;
use oxc::ast::ast::Program;
use oxc::ast::ast::Statement;
use oxc::ast_visit::VisitMut;
use oxc::semantic::Scoping;
use oxc::semantic::SemanticBuilder;
use oxc::syntax::symbol::SymbolId;

use crate::helpers::inliner::Inliner;
use crate::helpers::rewriter::Rewriter;
use crate::helpers::usage::HelperUsage;
use crate::helpers::usage::collect_usages;

/// Inline the used runtime helpers into `program`.
///
/// `module_name` is the resolved helper module name the oxc transformer used
/// for the runtime import specifiers. A no-op when the program contains no
/// helper import/require statements (which includes every non-inline run,
/// since the pass is only invoked in inline mode).
pub fn run<'a>(
    allocator: &'a Allocator,
    program: &mut Program<'a>,
    module_name: &str,
) -> anyhow::Result<()> {
    // Match the usage statements syntactically first: files without helper
    // usages return before paying for the semantic rebuild below.
    let usages: Vec<HelperUsage> = collect_usages(program, module_name);

    if usages.is_empty() {
        return Ok(());
    }

    // The transformed program's scoping is stale; rebuild it so binding
    // symbols of the usages can be read, then re-collect the usages to pick
    // up the correct symbol ids.
    let scoping: Scoping =
        SemanticBuilder::new().build(program).semantic.into_scoping();

    let usages: Vec<HelperUsage> = collect_usages(program, module_name);

    let mut inliner: Inliner<'_, '_> = Inliner {
        allocator,
        scoping: &scoping,
        claimed: HashSet::new(),
        resolved: HashMap::new(),
        emitted: Vec::new(),
    };

    // The first usage fixes the binding name (oxc's deconflicted uid); extra
    // usages are renamed to it.
    let mut duplicate_renames: HashMap<SymbolId, String> = HashMap::new();

    for usage in &usages {
        let final_name: String = match inliner.resolved.get(usage.helper.name) {
            | Some(final_name) => final_name.clone(),
            | None => {
                inliner.ensure_inlined(usage.helper, &usage.binding_name)?;
                usage.binding_name.clone()
            },
        };

        if final_name != usage.binding_name {
            let symbol_id: SymbolId = usage.binding_symbol_id.ok_or_else(
                || {
                    anyhow!(
                        "duplicate usage of helper `{}` binds `{}` without a symbol; its references cannot be renamed",
                        usage.helper.name,
                        usage.binding_name
                    )
                },
            )?;

            duplicate_renames.insert(symbol_id, final_name);
        }
    }

    if !duplicate_renames.is_empty() {
        let mut rewriter: Rewriter<'_, '_> = Rewriter {
            allocator,
            scoping: &scoping,
            renames: duplicate_renames,
            reset_spans: false,
        };

        rewriter.visit_program(program);
    }

    // Remove the usage statements and insert the inlined statements at the
    // top of the body (after directives, which live in `Program::directives`).
    let usage_indexes: HashSet<usize> = usages
        .iter()
        .map(|usage: &HelperUsage| usage.statement_index)
        .collect();

    let old_body: ArenaVec<'_, Statement<'_>> =
        std::mem::replace(&mut program.body, ArenaVec::new_in(&allocator));

    let mut new_body: ArenaVec<'_, Statement<'_>> =
        ArenaVec::new_in(&allocator);

    new_body.extend(inliner.emitted.drain(..));

    for (index, statement) in old_body.into_iter().enumerate() {
        if !usage_indexes.contains(&index) {
            new_body.push(statement);
        }
    }

    program.body = new_body;

    Ok(())
}
