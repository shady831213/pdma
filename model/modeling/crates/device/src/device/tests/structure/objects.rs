use super::support::{assert_layer_excludes, assert_unique_owner, source_root};
use std::{fs, io};

#[test]
fn queue_object_owns_one_closed_io_mode() -> io::Result<()> {
    // Given: the queue hardware object and full device source.
    let source = source_root();
    let queue = fs::read_to_string(source.join("queue/mod.rs"))?;

    // When/Then: semantic and exact I/O are a closed enum without a behavior trait.
    assert!(queue.contains("struct PdmaQueue"));
    assert!(queue.contains("enum QueueIo"));
    assert_layer_excludes(&source, &["dyn FrontendBehavior"])
}

#[test]
fn scheduler_object_uniquely_owns_runnable_state() -> io::Result<()> {
    // Given: the scheduler object source.
    let source = source_root();
    let owner = source.join("scheduler/mod.rs");
    let scheduler = fs::read_to_string(&owner)?;

    // When/Then: runnable FIFO state and transitions have one owner.
    assert!(scheduler.contains("struct Scheduler"));
    assert!(scheduler.contains("runnable: VecDeque<QueueId>"));
    assert!(scheduler.contains("fn enqueue("));
    assert!(scheduler.contains("fn next("));
    assert_unique_owner(&source, &owner, "runnable: VecDeque<QueueId>")
}

#[test]
fn exact_queue_uniquely_owns_ring_state() -> io::Result<()> {
    // Given: exact queue facade and state sources.
    let source = source_root();
    let owner = source.join("queue/exact/state.rs");

    // When/Then: active ring state is local to the exact queue object.
    assert!(
        fs::read_to_string(source.join("queue/exact/mod.rs"))?
            .contains("active: Option<ActiveQueue>")
    );
    assert!(fs::read_to_string(&owner)?.contains("struct ActiveQueue"));
    assert_unique_owner(&source, &owner, "struct ActiveQueue")?;
    assert_layer_excludes(&source, &["QueueControlState"])
}

#[test]
fn register_file_uniquely_owns_register_bank_storage() -> io::Result<()> {
    // Given: the three concrete register bank modules.
    let source = source_root();

    // When/Then: each storage type has exactly one owner.
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
    // Given: the synchronous device core and component modules.
    let source = source_root();
    let device = fs::read_to_string(source.join("device/mod.rs"))?;

    // When/Then: the top-level core directly contains every hardware object.
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
