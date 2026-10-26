use std::sync::Arc;

use oxc::allocator::{Allocator, CloneIn};
use oxc::ast::ast::Program;

use crate::_types::grammar::language::Language;
use crate::_types::grammar::source_type::SourceType;
use crate::helpers::resolve::{from_oxc_language, from_oxc_module_kind};
use crate::sourcemap::SourceMap;

struct AstOwner {
    allocator: Allocator,
    source: Arc<str>,
    // Constructor metadata. Public accessors report the program's resolved
    // state.
    language: Language,
    source_type: SourceType,
    source_map: Option<SourceMap>,
}

struct AstDependent<'cell> {
    program: Program<'cell>,
}

self_cell::self_cell!(
    struct AstInner {
        owner: AstOwner,

        #[covariant]
        dependent: AstDependent,
    }
);

impl AstInner {
    fn deep_clone(
        &self,
        source_map: Option<SourceMap>,
    ) -> AstInner {
        let owner: &AstOwner = self.borrow_owner();

        let capacity: usize = owner.allocator.used_bytes();

        let owner: AstOwner = AstOwner {
            allocator: Allocator::with_capacity(capacity),
            source: Arc::clone(&owner.source),
            language: owner.language,
            source_type: owner.source_type,
            source_map,
        };

        AstInner::new(owner, |owner: &AstOwner| AstDependent {
            program: self.borrow_dependent().program.clone_in(&owner.allocator),
        })
    }
}

impl Clone for AstInner {
    fn clone(&self) -> AstInner {
        self.deep_clone(self.borrow_owner().source_map.clone())
    }
}

// SAFETY: `self_cell` heap-boxes the owner and dependent together, and they
// move as one unit. `Allocator` (bumpalo `Bump`), `Arc<str>`, and the owned
// `Option<SourceMap>` are all `Send`. The `SourceMap` fields are
// `Cow`/`Vec`/`Box`. Thus no thread-affine pointer dangles when the inner value
// moves. `Arc` can share it between `Ast` handles.
unsafe impl Send for AstInner {}

// SAFETY: this impl needs two separate guarantees, and they are not the same.
//
// (a) The arena/`Allocator` is safe. It is only reachable through
//     `with_mut(&mut self, ..)`, an exclusive borrow. `with_mut` copies the
//     arena first when the `Arc` is shared. There is intentionally no public
//     `&Allocator` accessor, so safe code cannot bypass this invariant.
//
// (b) This justification does NOT cover `Program`'s own interior mutability.
//     `Program` exposes `pub node_id: Cell<NodeId>` and
//     `pub scope_id: Cell<Option<ScopeId>>`. Safe code can mutate these fields
//     through the shared reference returned by `program()`. Because `AstInner`
//     is `Sync`, `Ast` is `Sync` and `&Ast` is `Send`. Such a mutation from two
//     threads would be a data race. This impl is therefore sound ONLY under the
//     additional caller-side contract that the AST is shared across threads
//     read-only. No one mutates `Program`'s pub `Cell` fields through a shared
//     `&Ast`.
unsafe impl Sync for AstInner {}

/// An owned, self-referential AST that bundles its own allocator.
///
/// The optional [`Ast::map`] is the **input** source map. It describes how
/// this AST's source was produced from upstream originals. It says nothing
/// about the code generated from this AST.
///
/// `Program::source_text` must stay content-equal to [`Ast::source`].
/// [`Ast::with_mut`] can rewrite the tree but cannot change the source text.
/// Build a new `Ast` to change the source text.
///
/// Cloning is O(1) because clones share the same arena. [`Ast::with_mut`]
/// copies the arena into a fresh allocation when another clone is still
/// alive. Therefore, mutating a clone never mutates the original.
///
/// `Ast` is `Send + Sync`, so plugin futures can move owned handles or borrow
/// the AST across threads.
///
/// That `Sync` does not make concurrent mutation safe. `Program` exposes
/// interior-mutable `Cell` fields (`node_id`, `scope_id`). Safe code can
/// mutate these fields through the shared reference returned by
/// [`Ast::program`]. Sharing `&Ast` across threads is sound only under the
/// caller-side contract that it is used read-only. No thread may mutate
/// `Program`'s `Cell` fields through a shared `&Ast`. Doing so is a data race
/// and is forbidden.
pub struct Ast {
    inner: Arc<AstInner>,
}

impl Ast {
    /// Build an owned AST whose `Program::source_text` borrows the owner's source.
    ///
    /// `language` and `source_type` describe the requested grammar and module
    /// kind of the program `build` produces. `source_type` may be
    /// [`SourceType::Unambiguous`]. The parsed program resolves it.
    ///
    /// `build` must return a program whose `Program::source_text` is `source`.
    ///
    /// If present, `input_map` describes how `source` was produced from
    /// upstream originals. See [`Ast::map`].
    pub fn from_source<F>(
        source: Arc<str>,
        language: Language,
        source_type: SourceType,
        input_map: Option<SourceMap>,
        build: F,
    ) -> Ast
    where
        F: for<'a> FnOnce(&'a str, &'a Allocator) -> Program<'a>,
    {
        let owner: AstOwner = AstOwner {
            allocator: Allocator::default(),
            source,
            language,
            source_type,
            source_map: input_map,
        };

        let inner: AstInner = AstInner::new(owner, |owner: &AstOwner| {
            AstDependent { program: build(&owner.source, &owner.allocator) }
        });

        Ast { inner: Arc::new(inner) }
    }

    /// Fallible variant of [`Ast::from_source`] (parse / ESTree read-back).
    pub fn try_from_source<F, E>(
        source: Arc<str>,
        language: Language,
        source_type: SourceType,
        input_map: Option<SourceMap>,
        build: F,
    ) -> Result<Ast, E>
    where
        F: for<'a> FnOnce(&'a str, &'a Allocator) -> Result<Program<'a>, E>,
    {
        let owner: AstOwner = AstOwner {
            allocator: Allocator::default(),
            source,
            language,
            source_type,
            source_map: input_map,
        };

        let inner: AstInner = AstInner::try_new(owner, |owner: &AstOwner| {
            let program: Program<'_> = build(&owner.source, &owner.allocator)?;

            Ok(AstDependent { program })
        })?;

        Ok(Ast { inner: Arc::new(inner) })
    }

    /// Borrow the read-only program.
    pub fn program(&self) -> &Program<'_> {
        &self.inner.borrow_dependent().program
    }

    /// Borrow the owned source.
    pub fn source(&self) -> &Arc<str> {
        &self.inner.borrow_owner().source
    }

    /// The grammar of the parsed program.
    pub fn language(&self) -> Language {
        from_oxc_language(self.program().source_type)
    }

    /// The resolved module kind of the parsed program.
    ///
    /// `Unambiguous` is a parser input. A normally parsed AST resolves to
    /// `Script`, `CommonJS`, or `Module`.
    pub fn source_type(&self) -> SourceType {
        from_oxc_module_kind(self.program().source_type)
    }

    /// The input source map describing how `source` was produced from
    /// upstream originals, if any.
    pub fn map(&self) -> Option<&SourceMap> {
        self.inner.borrow_owner().source_map.as_ref()
    }

    /// Return a new AST that carries `source_map` as its input map.
    ///
    /// The new AST preserves the source text, language, and source type of
    /// `self`.
    pub fn with_input_map(
        &self,
        source_map: Option<SourceMap>,
    ) -> Ast {
        Ast { inner: Arc::new(self.inner.deep_clone(source_map)) }
    }

    /// The only path to the arena.
    ///
    /// The arena is copied first when another [`Ast`] handle is still alive,
    /// so mutation never affects a shared AST.
    ///
    /// The closure may rewrite the tree, but not the source text:
    /// `Program::source_text` must stay content-equal to [`Ast::source`].
    /// Debug builds assert this. Build a new `Ast` to change the source text.
    pub fn with_mut<R>(
        &mut self,
        f: impl for<'i> FnOnce(&'i Allocator, &mut Program<'i>) -> R,
    ) -> R {
        let inner: &mut AstInner = Arc::make_mut(&mut self.inner);

        inner.with_dependent_mut(|owner, dep| {
            let result: R = f(&owner.allocator, &mut dep.program);

            debug_assert_eq!(
                dep.program.source_text,
                owner.source.as_ref(),
                "with_mut must not replace Program::source_text; rebuild the Ast"
            );

            result
        })
    }
}

impl std::fmt::Debug for Ast {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        f.debug_struct("Ast").field("source_len", &self.source().len()).finish()
    }
}

impl Clone for Ast {
    /// Share the arena with the original AST.
    fn clone(&self) -> Ast {
        Ast { inner: Arc::clone(&self.inner) }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use oxc::parser::Parser;
    use oxc::span::SourceType as OxcSourceType;
    use oxc_sourcemap::SourceMapBuilder;

    use crate::_types::grammar::language::grammar_source_type;
    use crate::_types::grammar::source_type::with_module_kind;
    use crate::ast::parse::{ParseAstOptions, parse_ast};

    use super::*;

    fn build_program<'a>(
        source: &'a str,
        allocator: &'a Allocator,
    ) -> Program<'a> {
        Parser::new(allocator, source, OxcSourceType::ts()).parse().program
    }

    fn owner_from_code(code: &str) -> Ast {
        let source: Arc<str> = Arc::from(code);

        Ast::from_source(
            source,
            Language::TS,
            SourceType::Script,
            None,
            build_program,
        )
    }

    fn parse_code(
        code: &str,
        language: Option<Language>,
        source_type: Option<SourceType>,
    ) -> Ast {
        let options: ParseAstOptions = ParseAstOptions {
            cwd: PathBuf::new(),
            file: String::from("index.ts"),
            language,
            source_type,
            input_map: None,
        };

        parse_ast(code.to_string(), options).expect("valid source parses")
    }

    fn sample_map() -> SourceMap {
        let mut builder: SourceMapBuilder<'_> = SourceMapBuilder::default();

        builder.set_file("upstream.ts");

        let source_id: u32 =
            builder.add_source_and_content("upstream.ts", "const a = 1;");

        builder.add_token(0, 0, 0, 0, Some(source_id), None);

        builder.into_owned_sourcemap().into_inner()
    }

    #[test]
    fn test_from_source_builds_program() {
        let ast: Ast = owner_from_code("const a = 1;");

        assert!(!ast.program().body.is_empty());
        assert_eq!(ast.source().as_ref(), "const a = 1;");
        assert_eq!(ast.language(), Language::TS);
        assert_eq!(ast.source_type(), SourceType::Script);
        assert!(ast.program().source_type.is_typescript());
    }

    #[test]
    fn test_from_source_uses_semantic_source_metadata() {
        let ast: Ast = Ast::from_source(
            Arc::from("const a: number = <Button />;"),
            Language::TSX,
            SourceType::Module,
            None,
            |source: &str, allocator: &Allocator| {
                let raw: OxcSourceType = with_module_kind(
                    grammar_source_type(Language::TSX),
                    SourceType::Module,
                );

                Parser::new(allocator, source, raw).parse().program
            },
        );

        let raw: OxcSourceType = ast.program().source_type;

        assert_eq!(ast.language(), Language::TSX);
        assert_eq!(ast.source_type(), SourceType::Module);
        assert!(raw.is_typescript());
        assert!(raw.is_jsx());
        assert!(raw.is_module());
    }

    #[test]
    fn test_from_source_accepts_unambiguous_source_type() {
        let ast: Ast = Ast::from_source(
            Arc::from("export const a = 1;"),
            Language::TS,
            SourceType::Unambiguous,
            None,
            |source: &str, allocator: &Allocator| {
                let raw: OxcSourceType = with_module_kind(
                    grammar_source_type(Language::TS),
                    SourceType::Unambiguous,
                );

                Parser::new(allocator, source, raw).parse().program
            },
        );

        assert_eq!(ast.source_type(), SourceType::Module);
    }

    #[test]
    fn test_language_maps_every_grammar() {
        let cases: [(Language, &str); 5] = [
            (Language::JS, "const a = 1;"),
            (Language::TS, "const a: number = 1;"),
            (Language::DTS, "declare const a: string;"),
            (Language::JSX, "const x = <div>hi</div>;"),
            (Language::TSX, "const x = <div>hi</div>;"),
        ];

        for (language, code) in cases {
            let ast: Ast = parse_code(code, Some(language), None);

            assert_eq!(ast.language(), language, "grammar {language:?}");
        }
    }

    #[test]
    fn test_source_type_maps_every_resolved_module_kind() {
        let cases: [(SourceType, &str); 3] = [
            (SourceType::Script, "const a = 1;"),
            (SourceType::CommonJS, "const a = 1;"),
            (SourceType::Module, "export const a = 1;"),
        ];

        for (module_kind, code) in cases {
            let ast: Ast =
                parse_code(code, Some(Language::JS), Some(module_kind));

            assert_eq!(
                ast.source_type(),
                module_kind,
                "module kind {module_kind:?}"
            );
        }
    }

    #[test]
    fn test_unambiguous_parse_reports_the_resolved_module_kind() {
        let module: Ast = parse_code(
            "export const a = 1;",
            Some(Language::TS),
            Some(SourceType::Unambiguous),
        );

        assert_eq!(module.source_type(), SourceType::Module);

        let script: Ast = parse_code(
            "const a = 1;",
            Some(Language::TS),
            Some(SourceType::Unambiguous),
        );

        assert_eq!(script.source_type(), SourceType::Script);
    }

    #[test]
    fn test_program_exposes_the_raw_oxc_source_type() {
        let ast: Ast =
            parse_code("export const a = 1;", Some(Language::TSX), None);

        let raw: OxcSourceType = ast.program().source_type;

        assert!(raw.is_typescript());
        assert!(raw.is_jsx());
        assert!(raw.is_module());
        assert_eq!(ast.language(), Language::TSX);
        assert_eq!(ast.source_type(), SourceType::Module);
    }

    #[test]
    fn test_deep_clone_preserves_language_and_source_type() {
        let ast: Ast =
            parse_code("export const a = 1;", Some(Language::TSX), None);

        let mut clone: Ast = ast.clone();

        clone.with_mut(|_allocator: &Allocator, program: &mut Program<'_>| {
            program.body.clear();
        });

        assert!(!Arc::ptr_eq(&ast.inner, &clone.inner));
        assert_eq!(clone.language(), Language::TSX);
        assert_eq!(clone.source_type(), SourceType::Module);
    }

    #[test]
    fn test_with_input_map_preserves_language_and_source_type() {
        let ast: Ast =
            parse_code("export const a = 1;", Some(Language::TSX), None);

        let rebuilt: Ast = ast.with_input_map(Some(sample_map()));

        assert_eq!(rebuilt.language(), Language::TSX);
        assert_eq!(rebuilt.source_type(), SourceType::Module);
    }

    #[test]
    fn test_try_from_source_propagates_error() {
        let result: Result<Ast, &'static str> = Ast::try_from_source(
            Arc::from("const a = 1;"),
            Language::TS,
            SourceType::Script,
            None,
            |_source: &str, _allocator: &Allocator| Err("boom"),
        );

        assert_eq!(result.err(), Some("boom"));
    }

    #[test]
    fn test_try_from_source_builds_program() {
        let result: Result<Ast, &'static str> = Ast::try_from_source(
            Arc::from("const a = 1;"),
            Language::TS,
            SourceType::Script,
            None,
            |source: &str, allocator: &Allocator| {
                Ok(build_program(source, allocator))
            },
        );

        let ast: Ast = result.expect("valid source parses");

        assert!(!ast.program().body.is_empty());
    }

    #[test]
    fn test_program_source_text_borrows_owner_source() {
        let ast: Ast = owner_from_code("const a = 1;");

        assert_eq!(ast.program().source_text, ast.source().as_ref());
    }

    #[test]
    fn test_with_mut_can_mutate_program() {
        let mut ast: Ast = owner_from_code("const a = 1;");

        ast.with_mut(|_allocator: &Allocator, program: &mut Program<'_>| {
            program.body.clear();
        });

        assert!(ast.program().body.is_empty());
        assert_eq!(ast.program().source_text, ast.source().as_ref());
    }

    #[test]
    fn test_clone_round_trips_program() {
        let ast: Ast = owner_from_code("const a = 1;");

        let clone: Ast = ast.clone();

        assert_eq!(clone.program().source_text, ast.program().source_text);
        assert_eq!(clone.program().body.len(), ast.program().body.len());
        assert_eq!(clone.language(), ast.language());
        assert_eq!(clone.source_type(), ast.source_type());
    }

    #[test]
    fn test_clone_shares_source_arc() {
        let ast: Ast = owner_from_code("const a = 1;");

        let clone: Ast = ast.clone();

        assert!(Arc::ptr_eq(ast.source(), clone.source()));
    }

    #[test]
    fn test_clone_shares_inner_arena() {
        let ast: Ast = owner_from_code("const a = 1;");

        let clone: Ast = ast.clone();

        assert!(Arc::ptr_eq(&ast.inner, &clone.inner));
    }

    #[test]
    fn test_with_mut_on_clone_copies_on_write() {
        let ast: Ast = owner_from_code("const a = 1;");

        let mut clone: Ast = ast.clone();

        clone.with_mut(|_allocator: &Allocator, program: &mut Program<'_>| {
            program.body.clear();
        });

        assert!(!ast.program().body.is_empty());
        assert!(clone.program().body.is_empty());
        assert!(!Arc::ptr_eq(&ast.inner, &clone.inner));
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "source_text")]
    fn test_with_mut_rejects_replacing_source_text() {
        let mut ast: Ast = owner_from_code("const a = 1;");

        ast.with_mut(|allocator: &Allocator, program: &mut Program<'_>| {
            program.source_text = allocator.alloc_str("// changed");
        });
    }

    #[test]
    fn test_map_is_none_without_input_map() {
        let ast: Ast = owner_from_code("const a = 1;");

        assert!(ast.map().is_none());
    }

    #[test]
    fn test_from_source_carries_input_map() {
        let source: Arc<str> = Arc::from("const a = 1;");

        let map: SourceMap = sample_map();

        let ast: Ast = Ast::from_source(
            source,
            Language::TS,
            SourceType::Script,
            Some(map.clone()),
            build_program,
        );

        let carried: &SourceMap = ast.map().expect("input map attached");

        assert_eq!(carried.to_json_string(), map.to_json_string());
    }

    #[test]
    fn test_clone_carries_input_map() {
        let source: Arc<str> = Arc::from("const a = 1;");

        let map: SourceMap = sample_map();

        let ast: Ast = Ast::from_source(
            source,
            Language::TS,
            SourceType::Script,
            Some(map.clone()),
            build_program,
        );

        let clone: Ast = ast.clone();

        let carried: &SourceMap =
            clone.map().expect("clone carries the input map");

        assert_eq!(carried.to_json_string(), map.to_json_string());
    }

    #[test]
    fn test_with_input_map_rebuilds_with_new_map() {
        let ast: Ast = owner_from_code("const a = 1;");

        let map: SourceMap = sample_map();

        let rebuilt: Ast = ast.with_input_map(Some(map.clone()));

        let carried: &SourceMap =
            rebuilt.map().expect("rebuilt AST carries the new map");

        assert_eq!(carried.to_json_string(), map.to_json_string());
    }

    #[test]
    fn test_with_input_map_preserves_program_and_source() {
        let ast: Ast = owner_from_code("const a = 1;");

        let rebuilt: Ast = ast.with_input_map(Some(sample_map()));

        assert_eq!(rebuilt.program().body.len(), ast.program().body.len());

        assert_eq!(rebuilt.program().source_text, ast.source().as_ref());

        assert!(Arc::ptr_eq(rebuilt.source(), ast.source()));

        assert_eq!(rebuilt.language(), ast.language());
        assert_eq!(rebuilt.source_type(), ast.source_type());
    }

    #[test]
    fn test_with_mut_after_with_input_map_keeps_anchor() {
        let mut rebuilt: Ast =
            owner_from_code("const a = 1;").with_input_map(Some(sample_map()));

        rebuilt.with_mut(
            |_allocator: &Allocator, program: &mut Program<'_>| {
                program.body.clear();
            },
        );

        assert_eq!(rebuilt.program().source_text, rebuilt.source().as_ref());
    }

    #[test]
    fn test_with_input_map_clears_map() {
        let source: Arc<str> = Arc::from("const a = 1;");

        let ast: Ast = Ast::from_source(
            source,
            Language::TS,
            SourceType::Script,
            Some(sample_map()),
            build_program,
        );

        let rebuilt: Ast = ast.with_input_map(None);

        assert!(rebuilt.map().is_none());
    }

    #[test]
    fn test_debug_prints_source_length() {
        let ast: Ast = owner_from_code("const a = 1;");

        let debug: String = format!("{:?}", ast);

        assert!(debug.contains("Ast"));
        assert!(debug.contains("source_len"));
        assert!(debug.contains("12"));
    }

    #[test]
    fn test_ast_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}

        assert_send_sync::<Ast>();
    }
}
