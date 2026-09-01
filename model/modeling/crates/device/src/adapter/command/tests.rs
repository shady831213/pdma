use std::path::{Path, PathBuf};
use std::{fs, io};

fn rust_sources(root: &Path) -> io::Result<Vec<String>> {
    let mut pending = vec![PathBuf::from(root)];
    let mut sources = Vec::new();
    while let Some(path) = pending.pop() {
        for entry in fs::read_dir(path)? {
            let path = entry?.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs")
                && path.file_name().is_none_or(|name| name != "tests.rs")
                && !path
                    .components()
                    .any(|component| component.as_os_str() == "tests")
            {
                sources.push(fs::read_to_string(path)?);
            }
        }
    }
    Ok(sources)
}

#[test]
fn abi_conversion_produces_the_semantic_command_union() -> io::Result<()> {
    // Given: the stateless ABI-to-semantic command adapter sources.
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/adapter/command.rs");
    let modules = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/adapter/command");

    // When: typed input and output vocabulary is inspected.
    let mut combined = fs::read_to_string(source)?;
    combined.push_str(&rust_sources(&modules)?.join("\n"));

    // Then: ABI commands map to semantic requests without another command union.
    for required in [
        "AbiCommand",
        "CommandOperation",
        "CopyRequest",
        "impl ExactCommandContext",
        "VectorAddRequest",
    ] {
        assert!(
            combined.contains(required),
            "missing adapter type: {required}"
        );
    }
    assert!(!combined.contains("ExecutionCommand"));
    assert!(!combined.contains("ExecutionOperation"));
    Ok(())
}
