//! Prevents exact transport dependencies in the host semantic directory.

use std::path::{Path, PathBuf};
use std::{fs, io};

fn rust_files(root: &Path) -> io::Result<Vec<PathBuf>> {
    let mut pending = vec![root.to_path_buf()];
    let mut files = Vec::new();
    while let Some(path) = pending.pop() {
        for entry in fs::read_dir(path)? {
            let path = entry?.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                files.push(path);
            }
        }
    }
    Ok(files)
}

fn assert_layer_excludes(root: &Path, forbidden: &[&str]) -> io::Result<()> {
    let mut leaks = Vec::new();
    for path in rust_files(root)? {
        let source = fs::read_to_string(&path)?;
        let matches: Vec<_> = forbidden
            .iter()
            .filter(|pattern| source.contains(**pattern))
            .copied()
            .collect();
        if !matches.is_empty() {
            leaks.push((path, matches));
        }
    }
    assert!(
        leaks.is_empty(),
        "host semantic dependencies leaked: {leaks:#?}"
    );
    Ok(())
}

#[test]
fn host_semantic_directory_has_no_exact_transport_dependencies() -> io::Result<()> {
    // Given: the host-owned semantic driver source tree.
    let semantic = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/semantic");

    // When/Then: the semantic API does not name ABI or register transport types.
    assert_layer_excludes(&semantic, &["abi::", "crate::register"])
}
