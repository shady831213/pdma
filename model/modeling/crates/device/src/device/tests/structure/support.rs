use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::{fs, io};

pub(super) fn source_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

pub(super) fn rust_files(root: &Path) -> io::Result<Vec<PathBuf>> {
    if !root.exists() {
        return Ok(Vec::new());
    }
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

pub(super) fn production_rust_files(root: &Path) -> io::Result<Vec<PathBuf>> {
    rust_files(root).map(|files| {
        files
            .into_iter()
            .filter(|path| {
                path.file_name().is_none_or(|name| name != "tests.rs")
                    && !path
                        .components()
                        .any(|component| component.as_os_str() == "tests")
            })
            .collect()
    })
}

pub(super) fn assert_layer_excludes(root: &Path, forbidden: &[&str]) -> io::Result<()> {
    let mut leaks = Vec::new();
    for path in production_rust_files(root)? {
        let source = fs::read_to_string(&path)?;
        let matches = forbidden
            .iter()
            .filter(|pattern| source.contains(**pattern))
            .copied()
            .collect::<Vec<_>>();
        if !matches.is_empty() {
            leaks.push((path, matches));
        }
    }
    assert!(leaks.is_empty(), "cross-layer dependencies: {leaks:#?}");
    Ok(())
}

pub(super) fn assert_production_excludes(root: &Path, forbidden: &[&str]) -> io::Result<()> {
    let mut leaks = Vec::new();
    for path in production_rust_files(root)? {
        let source = fs::read_to_string(&path)?;
        let matches = forbidden
            .iter()
            .filter(|pattern| source.contains(**pattern))
            .copied()
            .collect::<Vec<_>>();
        if !matches.is_empty() {
            leaks.push((path, matches));
        }
    }
    assert!(leaks.is_empty(), "production dependency leaks: {leaks:#?}");
    Ok(())
}

pub(super) fn assert_unique_owner(root: &Path, owner: &Path, pattern: &str) -> io::Result<()> {
    let mut competing = Vec::new();
    for path in production_rust_files(root)? {
        if path != owner && fs::read_to_string(&path)?.contains(pattern) {
            competing.push(path);
        }
    }
    assert!(
        competing.is_empty(),
        "state owned outside {}: {competing:#?}",
        owner.display()
    );
    Ok(())
}

pub(super) fn assert_directory_contract(source: &Path) -> io::Result<()> {
    let expected = [
        "abi",
        "adapter",
        "device",
        "dma",
        "execution",
        "irq",
        "queue",
        "register_file",
        "retirement",
        "runtime",
        "scheduler",
        "trace",
    ]
    .map(str::to_owned)
    .into_iter()
    .collect::<BTreeSet<_>>();
    let actual = fs::read_dir(source)?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect::<BTreeSet<_>>();
    assert_eq!(actual, expected, "device architecture directories drifted");
    for retired in [
        "semantic",
        "adapter/abi",
        "runner",
        "dma_space",
        "hardware",
        "engine",
        "compat",
    ] {
        assert!(
            !source.join(retired).exists(),
            "retired directory: {retired}"
        );
    }
    Ok(())
}
