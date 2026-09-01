//! Prevents ownership drift between device behavior, ABI state, conversion, and runtime code.

use std::collections::BTreeSet;
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

fn production_rust_files(root: &Path) -> io::Result<Vec<PathBuf>> {
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

fn assert_production_layer_excludes(root: &Path, forbidden: &[&str]) -> io::Result<()> {
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

fn assert_unique_owner(root: &Path, owner: &Path, pattern: &str) -> io::Result<()> {
    let mut competing_owners = Vec::new();
    for path in rust_files(root)? {
        if path != owner && fs::read_to_string(&path)?.contains(pattern) {
            competing_owners.push(path);
        }
    }
    assert!(
        competing_owners.is_empty(),
        "state owned outside {}: {competing_owners:#?}",
        owner.display()
    );
    Ok(())
}

fn assert_directory_contract(source: &Path) -> io::Result<()> {
    let expected = [
        "abi",
        "adapter",
        "compat",
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
    for required in expected {
        assert!(
            source.join(&required).is_dir(),
            "missing device architecture directory: {required}"
        );
    }
    for retired in [
        "semantic",
        "adapter/abi",
        "runner",
        "dma_space",
        "hardware",
        "engine",
    ] {
        assert!(
            !source.join(retired).exists(),
            "retired device architecture directory remains: {retired}"
        );
    }
    Ok(())
}

#[test]
fn device_layers_preserve_two_model_ownership() -> io::Result<()> {
    // Given: the device's explicit architecture directories.
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");

    // When/Then: structure exposes hardware objects and wire ABI stays outside execution.
    assert_directory_contract(&source)?;
    assert_layer_excludes(
        &source.join("execution"),
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
    let device = source.join("device/mod.rs");

    // When/Then: one hardware top owns synchronous device state behind the runtime shell.
    let top = fs::read_to_string(device)?;
    assert!(top.contains("struct DeviceCore"));
    assert!(!source.join("runtime/core.rs").exists());
    assert!(!source.join("device.rs").exists());
    assert!(!top.contains("tokio::"));
    assert!(!top.contains("async fn"));
    assert!(!top.contains("Mutex<"));
    for object in [
        "queue",
        "dma",
        "scheduler",
        "execution",
        "retirement",
        "irq",
        "register_file",
    ] {
        assert_layer_excludes(&source.join(object), &["tokio::", "async fn", "Mutex<"])?;
    }
    Ok(())
}

#[test]
fn hardware_components_never_borrow_the_complete_device() -> io::Result<()> {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for object in [
        "queue",
        "dma",
        "scheduler",
        "execution",
        "retirement",
        "irq",
        "register_file",
    ] {
        assert_layer_excludes(
            &source.join(object),
            &[
                "crate::device::PdmaDevice",
                "impl PdmaDevice",
                "&mut PdmaDevice",
                "&PdmaDevice",
            ],
        )?;
    }
    Ok(())
}

#[test]
fn runtime_alone_owns_async_facades_and_locking() -> io::Result<()> {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let async_primitives = [
        "AsyncDeviceHandle",
        "RunnerShared",
        ".lock().await",
        "async fn",
        "tokio::",
    ];
    let mut leaks = Vec::new();
    for root in [
        "abi",
        "adapter",
        "compat",
        "device",
        "dma",
        "execution",
        "irq",
        "queue",
        "register_file",
        "retirement",
        "scheduler",
    ] {
        for path in production_rust_files(&source.join(root))? {
            let contents = fs::read_to_string(&path)?;
            let matches = async_primitives
                .iter()
                .filter(|pattern| contents.contains(**pattern))
                .copied()
                .collect::<Vec<_>>();
            if !matches.is_empty() {
                leaks.push((path, matches));
            }
        }
    }
    assert!(
        leaks.is_empty(),
        "async/runtime ownership leaked: {leaks:#?}"
    );
    for root in [
        "abi",
        "adapter",
        "device",
        "dma",
        "execution",
        "irq",
        "queue",
        "register_file",
        "retirement",
        "scheduler",
    ] {
        assert_production_layer_excludes(&source.join(root), &["crate::runtime"])?;
    }
    Ok(())
}

#[test]
fn hardware_pipeline_dependencies_flow_forward() -> io::Result<()> {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    assert_layer_excludes(
        &source.join("queue"),
        &["crate::device", "crate::execution", "crate::retirement"],
    )?;
    assert_layer_excludes(
        &source.join("execution"),
        &[
            "crate::device",
            "crate::queue",
            "crate::retirement",
            "crate::runtime",
        ],
    )?;
    assert_layer_excludes(
        &source.join("retirement"),
        &["crate::device", "crate::runtime"],
    )?;
    assert_layer_excludes(&source.join("irq"), &["crate::device", "crate::runtime"])
}

#[test]
fn queue_object_owns_one_closed_io_mode() -> io::Result<()> {
    // Given: the queue object and the complete device source tree.
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let queue = fs::read_to_string(source.join("queue/mod.rs"))?;

    // When/Then: one queue owns a closed semantic-or-exact I/O mode without dynamic dispatch.
    assert!(queue.contains("struct PdmaQueue"));
    assert!(queue.contains("enum QueueIo"));
    assert_layer_excludes(&source, &["dyn FrontendBehavior"])
}

#[test]
fn scheduler_object_owns_runnable_state() -> io::Result<()> {
    // Given: the scheduler object source tree.
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let scheduler = fs::read_to_string(source.join("scheduler/mod.rs"))?;

    // When/Then: runnable FIFO state and transitions belong to one synchronous object.
    assert!(scheduler.contains("struct Scheduler"));
    assert!(scheduler.contains("runnable: VecDeque<QueueId>"));
    assert!(scheduler.contains("fn enqueue("));
    assert!(scheduler.contains("fn next("));
    assert_unique_owner(
        &source,
        &source.join("scheduler/mod.rs"),
        "runnable: VecDeque<QueueId>",
    )
}

#[test]
fn exact_queue_owns_its_ring_state() -> io::Result<()> {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let frontend = fs::read_to_string(source.join("queue/exact/mod.rs"))?;
    let state = fs::read_to_string(source.join("queue/exact/state.rs"))?;

    assert!(frontend.contains("active: Option<ActiveQueue>"));
    assert!(state.contains("struct ActiveQueue"));
    assert_unique_owner(
        &source,
        &source.join("queue/exact/state.rs"),
        "struct ActiveQueue",
    )?;
    assert_layer_excludes(&source, &["QueueControlState"])
}

#[test]
fn register_file_owns_all_register_bank_storage() -> io::Result<()> {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for (path, symbol) in [
        ("register_file/global.rs", "struct GlobalBank"),
        ("register_file/queue.rs", "struct QueueBank"),
        ("register_file/irq.rs", "struct IrqBank"),
    ] {
        let owner = source.join(path);
        assert!(fs::read_to_string(&owner)?.contains(symbol));
        assert_unique_owner(&source, &owner, symbol)?;
    }
    Ok(())
}

#[test]
fn device_core_directly_owns_hardware_components() -> io::Result<()> {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let device = fs::read_to_string(source.join("device/mod.rs"))?;

    for field in [
        "epoch: DeviceEpoch",
        "queues: QueueBlock",
        "scheduler: Scheduler",
        "execution: ExecutionUnit",
        "retirement: RetirementUnit",
        "irq: PdmaIrqController",
        "trace: TraceSink",
        "dma: PdmaMemoryMappings",
        "registers: RegisterFile",
    ] {
        assert!(device.contains(field), "DeviceCore does not own {field}");
    }
    for (path, symbol) in [
        ("queue/mod.rs", "struct QueueBlock"),
        ("dma/mod.rs", "struct PdmaMemoryMappings"),
        ("scheduler/mod.rs", "struct Scheduler"),
        ("execution/mod.rs", "struct ExecutionUnit"),
        ("retirement/mod.rs", "struct RetirementUnit"),
        ("irq/controller.rs", "struct PdmaIrqController"),
        ("register_file/mod.rs", "struct RegisterFile"),
    ] {
        assert!(
            fs::read_to_string(source.join(path))?.contains(symbol),
            "hardware object {symbol} is not owned by {path}"
        );
    }
    assert!(!source.join("queue/exact/device.rs").exists());
    assert_layer_excludes(&source, &["DeviceExecution", "NativeExecution"])
}
