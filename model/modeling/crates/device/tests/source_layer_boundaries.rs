//! Prevents ownership drift between device behavior, ABI state, conversion, and runtime code.

use std::path::{Path, PathBuf};
use std::{fs, io};

fn rust_files(root: &Path) -> io::Result<Vec<PathBuf>> {
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
        "cross-model device dependencies: {leaks:#?}"
    );
    Ok(())
}

fn assert_directory_contract(source: &Path) {
    for required in ["engine", "abi", "adapter", "runtime", "compat"] {
        assert!(
            source.join(required).is_dir(),
            "missing device architecture directory: {required}"
        );
    }
    for retired in ["semantic", "adapter/abi", "runner", "dma_space", "trace"] {
        assert!(
            !source.join(retired).exists(),
            "retired device architecture directory remains: {retired}"
        );
    }
}

#[test]
fn device_layers_preserve_two_model_ownership() -> io::Result<()> {
    // Given: the device's explicit architecture directories.
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");

    // When/Then: structure is explicit and wire ABI types stay outside the execution engine.
    assert_directory_contract(&source);
    assert_layer_excludes(
        &source.join("engine"),
        &[
            "pdma_model_contract::abi",
            "crate::adapter",
            "ExecutionCommand",
            "ExecutionOperation",
        ],
    )?;
    assert_layer_excludes(
        &source.join("adapter"),
        &[
            "struct RegisterBanks",
            "struct QueueControlState",
            "struct PdmaMemoryMappings",
            "struct MmioRegisterEndpoint",
        ],
    )
}

#[test]
fn runtime_wraps_one_synchronous_hardware_device() -> io::Result<()> {
    // Given: the device source tree and its async runtime wrapper.
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let hardware = source.join("hardware");
    let core = fs::read_to_string(source.join("runtime/core.rs"))?;

    // When/Then: one hardware top owns synchronous device state behind the runtime shell.
    assert!(
        hardware.is_dir(),
        "missing hardware-object device directory"
    );
    let top = fs::read_to_string(hardware.join("mod.rs"))?;
    assert!(top.contains("struct PdmaDevice"));
    assert!(core.contains("device: PdmaDevice"));
    assert_layer_excludes(&hardware, &["tokio::", "async fn", "Mutex<"])
}

#[test]
fn hardware_queue_owns_one_closed_io_mode() -> io::Result<()> {
    // Given: the hardware queue object and the complete device source tree.
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let queue = fs::read_to_string(source.join("hardware/queue.rs"))?;

    // When/Then: one queue owns a closed semantic-or-exact I/O mode without dynamic dispatch.
    assert!(queue.contains("struct PdmaQueue"));
    assert!(queue.contains("enum QueueIo"));
    assert_layer_excludes(&source, &["dyn FrontendBehavior"])
}

#[test]
fn hardware_scheduler_owns_runnable_state() -> io::Result<()> {
    // Given: the hardware-object source tree.
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let scheduler = fs::read_to_string(source.join("hardware/scheduler.rs"))?;

    // When/Then: runnable FIFO state and transitions belong to one synchronous object.
    assert!(scheduler.contains("struct Scheduler"));
    assert!(scheduler.contains("runnable: VecDeque<QueueId>"));
    assert!(scheduler.contains("fn enqueue("));
    assert!(scheduler.contains("fn next("));
    assert_layer_excludes(&source.join("engine"), &["runnable: VecDeque<QueueId>"])
}
