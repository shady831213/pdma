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
fn execution_uses_semantic_commands_without_a_third_command_model() -> io::Result<()> {
    // Given: every source file owned by the execution object.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/execution");

    // When: its command vocabulary is inspected.
    let combined = rust_sources(&root)?.join("\n");

    // Then: semantic operations are executed directly without a duplicate command union.
    assert!(combined.contains("CommandOperation"));
    assert!(!combined.contains("ExecutionCommand"));
    assert!(!combined.contains("ExecutionOperation"));
    Ok(())
}
