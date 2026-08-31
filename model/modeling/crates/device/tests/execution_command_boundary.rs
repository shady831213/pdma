//! Guards the two-model command boundary.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

fn rust_sources(root: &Path) -> io::Result<Vec<String>> {
    let mut pending = vec![PathBuf::from(root)];
    let mut sources = Vec::new();
    while let Some(path) = pending.pop() {
        for entry in fs::read_dir(path)? {
            let path = entry?.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                sources.push(fs::read_to_string(path)?);
            }
        }
    }
    Ok(sources)
}

#[test]
fn engine_executes_semantic_commands_without_a_third_command_model() -> io::Result<()> {
    // Given: every source file in the device execution engine.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/engine");
    let sources = rust_sources(&root)?;

    // When: the engine's command vocabulary is inspected.
    let combined = sources.join("\n");

    // Then: semantic operations are executed directly and no duplicate command union exists.
    assert!(combined.contains("CommandOperation"));
    assert!(!combined.contains("ExecutionCommand"));
    assert!(!combined.contains("ExecutionOperation"));
    Ok(())
}

#[test]
fn abi_command_conversion_produces_the_semantic_command_union() -> io::Result<()> {
    // Given: the stateless ABI-to-semantic command adapter.
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/adapter/command.rs");
    let modules = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/adapter/command");

    // When: its typed input and output vocabulary is inspected.
    let mut source = fs::read_to_string(path)?;
    source.push_str(&rust_sources(&modules)?.join("\n"));

    // Then: it maps ABI commands to semantic requests without inventing another union.
    for required in [
        "AbiCommand",
        "CommandOperation",
        "CopyRequest",
        "VectorAddRequest",
    ] {
        assert!(
            source.contains(required),
            "missing adapter type: {required}"
        );
    }
    assert!(!source.contains("ExecutionCommand"));
    assert!(!source.contains("ExecutionOperation"));
    Ok(())
}
