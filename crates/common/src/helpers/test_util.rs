use std::fs;
use std::path::PathBuf;

use tempfile::TempDir;

pub fn temp_dir() -> TempDir {
    tempfile::tempdir().expect("temp dir creatable")
}

pub fn path_buf(dir: &TempDir) -> PathBuf {
    dir.path().to_path_buf()
}

pub fn write_file(
    dir: &TempDir,
    name: &str,
    contents: &str,
) -> PathBuf {
    let path: PathBuf = dir.path().join(name);

    fs::write(&path, contents).expect("temp file writable");

    path
}
