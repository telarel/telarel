set shell := ["bash", "-cu"]
set windows-shell := ["pwsh", "-Command"]

oxfmt := "pnpm exec oxfmt"
oxlint := "pnpm exec oxlint"
napi := "pnpm exec napi"
tsdown := "pnpm exec tsdown"
vitest := "pnpm exec vitest"

publish_js_dev := "pnpm publish --no-git-checks --tag dev --access public"
publish_js := "pnpm publish --access public"

telarel := "packages/telarel"

test_telarel := "tests/telarel"

bench_rs := "benchmarks/rust"
bench_js := "benchmarks/javascript"

# Default action
_:
    just --list -u

# Initialize
init:
    rustup target add aarch64-apple-darwin
    rustup target add x86_64-apple-darwin

    rustup target add aarch64-unknown-linux-gnu
    rustup target add aarch64-unknown-linux-musl
    rustup target add x86_64-unknown-linux-gnu
    rustup target add x86_64-unknown-linux-musl

    rustup target add aarch64-pc-windows-msvc
    rustup target add x86_64-pc-windows-msvc

    rustup target add wasm32-wasip1-threads

# Install
i:
    pnpm install

# Format Rust code
fmt-rs:
    cargo fmt

# Format JavaScript code
fmt-js:
    {{oxfmt}}

# Format code
fmt: fmt-rs fmt-js

# Lint code with ls-lint
ls-lint:
    ls-lint -config ./.ls-lint.yaml

# Lint code with ls-lint
lslint: ls-lint

# Lint code with typos
typos:
    typos

# Lint Rust code
lint-rs:
    cargo clippy

# Lint JavaScript code with type check
lint-js:
    {{oxlint}} --fix --fix-suggestions --fix-dangerously

# Lint code
lint: lint-rs lint-js

# Create NPM dirs
create-npm-dirs:
    cd ./{{telarel}} && {{napi}} create-npm-dirs

# Build NAPI binding
build-rs:
    node ./{{telarel}}/scripts/build.ts

# Build JavaScript packages
build-js:
    cd ./{{telarel}} && {{tsdown}}

# Build binding and JavaScript packages
build: build-rs build-js

# Build NAPI binding for distribution (all platforms + optimization)
dist:
    node ./{{telarel}}/scripts/dist.ts

# Run Rust test
test-rs:
    cargo test -- --nocapture

# Test JavaScript code with native binding
[env("VITE_CONFIG_NATIVE_IGNORE_WARNING", "true")]
test-js-native:
    cd ./{{test_telarel}} && {{vitest}} run

# Test JavaScript code under WASI
[env("NODE_OPTIONS", "--disable-warning=ExperimentalWarning")]
[env("VITE_CONFIG_NATIVE_IGNORE_WARNING", "true")]
test-js-wasm:
    cd ./{{test_telarel}} && {{vitest}} run --config vitest.wasm.config.ts

# Test JavaScript code
test-js: test-js-native test-js-wasm

# Rust test
test: test-rs test-js

# Run Rust benchmarks
bench-rs:
    cargo bench -p telarel_bench

# Run JavaScript benchmarks
[env("VITE_CONFIG_NATIVE_IGNORE_WARNING", "true")]
bench-js:
    cd ./{{bench_js}} && {{vitest}} bench --run

# Run benchmarks
bench: bench-rs bench-js

# Check Rust code
check-rs: fmt-rs lint-rs test-rs

# Check JavaScript code
check-js: fmt-js lint-js test-js

# Check code
check: fmt ls-lint typos lint test

# Check code (Full mode)
check-full: create-npm-dirs build fmt ls-lint typos lint test

# Bump package versions
ver VERSION:
    pnpm version \
        --no-git-tag-version \
        --no-commit-hooks \
        --no-git-checks \
        --allow-same-version \
        --recursive \
        {{VERSION}}

# Publish a Rust crate (with optional flags)
publish-rs-crate PKG CMD:
    cargo publish -p {{PKG}} {{CMD}}

# Publish all Rust crates as dry-run (topological order)
publish-rs-try:
    just publish-rs-crate telarel_common "--dry-run"
    just publish-rs-crate telarel_plugin "--dry-run"
    just publish-rs-crate telarel_plugin_transform "--dry-run"
    just publish-rs-crate telarel_core "--dry-run"
    just publish-rs-crate telarel "--dry-run"

# Publish all Rust crates (topological order)
publish-rs:
    just publish-rs-crate telarel_common ""
    just publish-rs-crate telarel_plugin ""
    just publish-rs-crate telarel_plugin_transform ""
    just publish-rs-crate telarel_core ""
    just publish-rs-crate telarel ""

# Publish JS packages to all platforms (npm dirs -> root)
publish-js-pkgs PKG CMD:
    cd ./{{PKG}}/npm/darwin-arm64 && {{CMD}}
    cd ./{{PKG}}/npm/darwin-x64 && {{CMD}}
    cd ./{{PKG}}/npm/linux-arm64-gnu && {{CMD}}
    cd ./{{PKG}}/npm/linux-arm64-musl && {{CMD}}
    cd ./{{PKG}}/npm/linux-x64-gnu && {{CMD}}
    cd ./{{PKG}}/npm/linux-x64-musl && {{CMD}}
    cd ./{{PKG}}/npm/win32-arm64-msvc && {{CMD}}
    cd ./{{PKG}}/npm/win32-x64-msvc && {{CMD}}
    cd ./{{PKG}}/npm/wasm32-wasi && {{CMD}}
    cd ./{{PKG}} && {{CMD}}

# Publish JS packages with dev tag as dry-run
publish-js-dev-try:
    just publish-js-pkgs {{telarel}} "{{publish_js_dev}} --dry-run"

# Publish JS packages with dev tag
publish-js-dev:
    just publish-js-pkgs {{telarel}} "{{publish_js_dev}}"

# Publish JS packages as dry-run
publish-js-try:
    just publish-js-pkgs {{telarel}} "{{publish_js}} --dry-run"

# Publish JS packages
publish-js:
    just publish-js-pkgs {{telarel}} "{{publish_js}}"

# Clean
clean:
    cargo clean
    pnpm clean
