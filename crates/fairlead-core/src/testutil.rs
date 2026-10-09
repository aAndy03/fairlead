//! Test-only helpers. Filesystem tests use `tempfile` directories only.
use camino::{Utf8Path, Utf8PathBuf};
use std::fs;

/// A temporary directory and its UTF-8 path. Keep the guard alive for the whole test.
pub fn utf8_tmp() -> (tempfile::TempDir, Utf8PathBuf) {
    let td = tempfile::tempdir().expect("create tempdir");
    let p = Utf8PathBuf::from_path_buf(td.path().to_path_buf()).expect("utf-8 temp path");
    (td, p)
}

/// Write `text` to `dir/rel`, creating parent directories.
pub fn write_file(dir: &Utf8Path, rel: &str, text: &str) {
    write_bytes(dir, rel, text.as_bytes());
}

/// Write raw bytes to `dir/rel`, creating parent directories.
pub fn write_bytes(dir: &Utf8Path, rel: &str, bytes: &[u8]) {
    let full = dir.join(rel);
    if let Some(parent) = full.parent() {
        fs::create_dir_all(parent).expect("create parent dirs");
    }
    fs::write(full.as_std_path(), bytes).expect("write file");
}

use crate::discover::{Edition, Pin, Root, RootSource};

/// A minimal valid root control file.
pub const ROOT_CONTROL: &str = "[fairlead]\nedition = 1\nroot = true\nscope = \"root\"\n";
/// A minimal valid child control file.
pub const CHILD_CONTROL: &str = "[fairlead]\nscope = \"web\"\n";

/// A `Root` for tests that do not run discovery.
pub fn test_root(dir: &Utf8Path) -> Root {
    Root {
        dir: dir.to_path_buf(),
        folder: "fairlead".to_string(),
        source: RootSource::Flag,
        edition: Edition(1),
        pin: Pin {
            version: None,
            edition: Edition(1),
        },
        config_hash: crate::hash::hash_raw(b""),
    }
}
