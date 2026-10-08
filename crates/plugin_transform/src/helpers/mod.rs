pub mod inliner;
pub mod rewriter;
pub mod sources;
pub mod usage;

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
/// for the runtime import specifiers. Returns `Ok(false)` when the program
/// contains no helper import/require statements (which includes every
/// non-inline run, since the pass is only invoked in inline mode) and the
/// program is left untouched; `Ok(true)` reports that edits were applied.
pub fn run<'a>(
    allocator: &'a Allocator,
    program: &mut Program<'a>,
    module_name: &str,
) -> anyhow::Result<bool> {
    // Match the usage statements syntactically first: files without helper
    // usages return before paying for the semantic rebuild below.
    let usages: Vec<HelperUsage> = collect_usages(program, module_name);

    if usages.is_empty() {
        return Ok(false);
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

    Ok(true)
}

#[cfg(test)]
mod tests {
    use oxc::allocator::Allocator;
    use oxc::ast::ast::Program;
    use oxc::parser::Parser;
    use oxc::parser::ParserReturn;
    use oxc::span::SourceType;

    use telarel_common::{CodegenOptions, codegen};

    use crate::helpers::sources::{DEPENDENCY_HELPERS, HELPERS, HelperSource};
    use crate::options::helper_loader::DEFAULT_HELPER_MODULE_NAME;

    use super::*;

    const ALL_HELPERS: [&[HelperSource]; 2] = [HELPERS, DEPENDENCY_HELPERS];

    fn every_helper() -> impl Iterator<Item = &'static HelperSource> {
        ALL_HELPERS.into_iter().flat_map(|table: &[HelperSource]| table)
    }

    fn parse_program<'a>(
        allocator: &'a Allocator,
        source: &'a str,
        source_type: SourceType,
    ) -> Program<'a> {
        let parsed: ParserReturn<'a> =
            Parser::new(allocator, source, source_type).parse();

        assert!(
            !parsed.fatal_error && !parsed.diagnostics.has_errors(),
            "synthetic program must parse: {source}"
        );

        parsed.program
    }

    fn codegen_program(program: &Program<'_>) -> String {
        codegen(CodegenOptions { file: "index.mjs", program }).code
    }

    #[test]
    fn test_all_helpers_inline_in_one_program() {
        let mut source: String = String::new();

        for helper in every_helper() {
            source.push_str(&format!(
                "import _{0} from \"{DEFAULT_HELPER_MODULE_NAME}/helpers/{0}\";\n",
                helper.name
            ));
        }

        let allocator: Allocator = Allocator::default();

        let mut program: Program<'_> =
            parse_program(&allocator, &source, SourceType::mjs());

        let inlined: bool =
            run(&allocator, &mut program, DEFAULT_HELPER_MODULE_NAME)
                .expect("every helper inlines together");

        assert!(inlined, "the combined program must be edited");

        let code: String = codegen_program(&program);

        // At this scale a helper can be pulled into an earlier helper's
        // dependency closure under a fresh uid before its own direct import
        // is processed, so exact per-name assertions belong to the
        // plugin-level trigger tests in `plugin.rs`. Here the load-bearing
        // invariant is that every runtime specifier was consumed and
        // deconfliction did not panic or drop helpers.
        assert!(
            !code.contains(DEFAULT_HELPER_MODULE_NAME),
            "no runtime specifier may survive: {code}"
        );
        assert!(
            !code.contains("import "),
            "no helper import may survive: {code}"
        );
        assert!(
            code.matches("function _").count() > every_helper().count(),
            "every helper and its closure must emit a function: {code}"
        );
    }
}
