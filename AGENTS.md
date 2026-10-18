# Telarel

Telarel is an extensible JavaScript compiler written in Rust.

## Documentation Map

- `docs/content` — public, published docs. Source of truth for user-facing API and behavior.
- `crates/<crate>/AGENTS.md` — crate-local architecture, invariants, and where things live.

Read the one for the crate you are changing.

## Architecture

This repository is a TypeScript/Rust monorepo.

### Rust Crates

| Path                      | Description                                            |
| ------------------------- | ------------------------------------------------------ |
| `crates/common`           | Share code across different stage                      |
| `crates/plugin`           | The plugin module to provide structs, traits and types |
| `crates/plugin_transform` | The transform plugin                                   |
| `crates/core`             | The pipeline orchestrator                              |
| `crates/binding`          | NAPI bindings for the compiler                         |
| `crates/telarel`          | Re-export public facing API                            |

### TypeScript Packages

| Path               | Description                         |
| ------------------ | ----------------------------------- |
| `packages/telarel` | JavaScript package for the compiler |

### TypeScript Tests

| Path            | Description                      |
| --------------- | -------------------------------- |
| `tests/telarel` | JavaScript test for the compiler |

### Benchmarks

| Path                    | Description              |
| ----------------------- | ------------------------ |
| `benchmarks/rust`       | Benchmark with Criterion |
| `benchmarks/javascript` | Benchmark with Vitest    |

## Dependency Boundaries

| Crate              | May depend on                          |
| ------------------ | -------------------------------------- |
| `common`           | /                                      |
| `plugin`           | `common`                               |
| `plugin_transform` | `common`, `plugin`                     |
| `core`             | `common`, `plugin`                     |
| `binding`          | `common`, `plugin`, `core`, `plugin_*` |
| `telarel`          | `common`, `plugin`, `core`             |

Do not introduce circular dependencies.

## Code Standards

### Languages

This repository contains Rust and TypeScript/JavaScript code.

#### Rust

- use idiomatic Rust
- use explicit types for variables
- use explicit types for exported APIs
- prefer pure functions where practical
- prefer small, composable utilities
- avoid mutation unless it is required for correctness, performance or integration

#### TypeScript

- use explicit types for variables
- use explicit types for exported APIs
- avoid `any` unless there is a concrete and documented reason

### General

- preserve the surrounding code style
- reuse existing utilities and abstractions before introducing new ones

### Comments

- comments explain **why**, not **what**. The code already says what it does
- if the `why` cannot be stated briefly, the code may need refactoring, not a longer comment
- delete comments that only translate code into prose:
    - restated names
    - obvious control flow
    - per-line walkthroughs
- if a comment can be shortened to 1–2 lines without losing intent, shorten it
- keep comments only for non-obvious intent:
    - invariants
    - trade-offs
    - workarounds
    - external constraints
    - surprising behavior
    - or reasons a simpler approach was rejected
- public API doc comments state the contract only, never a step-by-step narration
- document every public struct, field, and function with at least one line stating its purpose or contract; a restated name or implementation narration is not documentation

> For this rule, "public" means reachable from the crate or package entry point; `pub(crate)`, `pub(super)`, `pub(in ...)`, and `pub` items inside private modules are private

## Editing Rules

When modifying code:

- always ask before changing public API semantics
- if uncertain about intended behavior, ask directly, do not guess
- prefer reading tests as source of truth

### Documentation

- if public behavior changes, update the tests and `docs/content`
- if internal architecture or invariants change, update the affected crate `AGENTS.md`

`docs/content` documents public behavior only. `crates/<crate>/AGENTS.md` documents internal architecture only. Do not duplicate public API semantics into agent docs; link to `docs/content` instead.

Update `docs/content` only when a change alters what users can observe:

- exported functions, types, options, defaults, or result shapes
- the plugin contract, hook semantics, ordering, or lifecycle
- user-triggerable errors or failure behavior
- installation, package entry points, or usage examples

Do not update `docs/content` for:

- internal refactors, private renames, or module moves
- performance-only changes
- test-only changes or dependency bumps
- internal invariants or file layout

Update the affected `crates/<crate>/AGENTS.md` when a change alters internal architecture, invariants, file layout, or test commands a future contributor needs.

If unsure whether behavior is public, check existing docs and tests and ask instead of guessing. When code and docs conflict, tests are the source of truth for behavior; fix the docs to match the intended public contract.

## Testing Rules

- add tests when adding new behavior
- update tests when behavior changes
- keep test style consistent with nearby tests

### Integrity

- do not delete failing test to make the suite pass
- do not weaken assertions to make the suite pass
- do not change expected output without understanding why the behavior changed
- do not remove coverage because the implementation is difficult to test

If an existing test fails after a change, determine whether:

1. the implementation is incorrect, or
2. the expected behavior intentionally changed

Only update the test in the second case.

## Tooling and Workflow

The repository uses:

- Cargo
- Node.js
- pnpm
- just
- ls-lint
- typos-cli

`just` is the preferred task runner over the lower-level tools.

Before running an task, inspect the available commands:

```sh
just
```

After editing any Rust crate, rebuild with `just build-rs`.
Therefore, the artifact distribution runs. Avoid invoke `napi build` directly.

## What NOT to Do

- invent APIs, files, modules, or behavior
- assume unsupported features exist
- violate dependency boundaries
- introduce circular dependencies
- add unnecessary dependencies
- refactor unrelated code during a focused change
- modify generated artifacts directly when a generation workflow exists
- write comments that restate what the code does or paraphrase function names
- leave multi-paragraph comments narrating implementation steps
- introduce unnecessary mutation
- migrate tooling without an explicit requirement
- introduce a second package manager
- commit or push Git changes unless explicitly requested
