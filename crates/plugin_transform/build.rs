use std::path::PathBuf;

const RUNTIME_PACKAGE_DIR: &str = "node_modules/@oxc-project/runtime";
const RUNTIME_HELPERS_ENV: &str = "TELAREL_RUNTIME_HELPERS_DIR";

/// Resolve the ESM helper directory of the installed runtime package.
fn resolve_esm_helpers_dir() -> PathBuf {
    let workspace_root: PathBuf = workspace_root::get_workspace_root();

    let package_dir: PathBuf = workspace_root.join(RUNTIME_PACKAGE_DIR);

    let manifest_path: PathBuf = package_dir.join("package.json");

    if !package_dir.is_dir() || !manifest_path.is_file() {
        panic!(
            "runtime helpers: {} is not an installed @oxc-project/runtime \
             package; run `pnpm install` at the workspace root",
            package_dir.display()
        );
    }

    package_dir.join("src/helpers/esm")
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let helpers_dir: PathBuf = resolve_esm_helpers_dir();

    println!("cargo:rustc-env={RUNTIME_HELPERS_ENV}={}", helpers_dir.display());

    println!("cargo:rerun-if-changed={}", helpers_dir.display());
}
