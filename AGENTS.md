# Telarel

Telarel is an extensible JavaScript compiler written in Rust.

For more details, refer to `./ARCHITECTURE.md`.

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

## Editing Rules

When modifying code:

- always ask before changing public API semantics
- if behavior changes, update tests and docs (like `./ARCHITECTURE.md`) accordingly

If uncertain about intended behavior:

- ask directly, do not guess
- prefer reading tests as source of truth

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
- refactor unrelated code during a focuesd change
- modify generated artifacts directly when a generation workflow exists
- migrate tooling without an explicit requirement
- intoruce a second package manager
- introduce unnecessary mutation
- commit or push Git changes unless explicitly requested
