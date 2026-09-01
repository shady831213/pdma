use super::support::{
    assert_directory_contract, assert_layer_excludes, assert_production_excludes,
    production_rust_files, source_root,
};
use std::{fs, io};

#[test]
fn device_layers_preserve_semantic_and_abi_ownership() -> io::Result<()> {
    // Given: the device's explicit hardware-object directories.
    let source = source_root();

    // When: the execution and adapter dependency directions are inspected.
    assert_directory_contract(&source)?;

    // Then: execution consumes semantic commands while ABI conversion remains outside it.
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
fn runtime_wraps_one_synchronous_hardware_core() -> io::Result<()> {
    // Given: the synchronous core and async runtime sources.
    let source = source_root();
    let core = fs::read_to_string(source.join("device/mod.rs"))?;

    // When: ownership markers are inspected across the two layers.
    let runtime_state = fs::read_to_string(source.join("runtime/state.rs"))?;

    // Then: async coordination and observation live only in runtime state.
    assert!(core.contains("struct DeviceCore"));
    for forbidden in [
        "tokio::",
        "async fn",
        "Mutex<",
        "ObservationGate",
        "RegisterObserver",
        "register_gate",
        "shutdown_requested",
        "wake_generation",
    ] {
        assert!(
            !core.contains(forbidden),
            "core owns runtime state: {forbidden}"
        );
    }
    for required in [
        "register_gate",
        "shutdown_requested",
        "wake_generation",
        "generation",
    ] {
        assert!(
            runtime_state.contains(required),
            "runtime missing {required}"
        );
    }
    for retired in [
        "compat",
        "test_support.rs",
        "runtime/manual.rs",
        "runtime/test_support.rs",
    ] {
        assert!(
            !source.join(retired).exists(),
            "retired source remains: {retired}"
        );
    }
    Ok(())
}

#[test]
fn runtime_alone_owns_async_facades_locks_and_observers() -> io::Result<()> {
    // Given: every production file outside the runtime directory.
    let source = source_root();
    let roots = [
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
    ];
    let mut leaks = Vec::new();

    // When: async primitives and runtime dependencies are searched.
    for root in roots {
        for path in production_rust_files(&source.join(root))? {
            let contents = fs::read_to_string(&path)?;
            let matches = [
                "AsyncDeviceHandle",
                "RunnerShared",
                ".lock().await",
                "async fn",
                "tokio::",
            ]
            .into_iter()
            .filter(|pattern| contents.contains(pattern))
            .collect::<Vec<_>>();
            if !matches.is_empty() {
                leaks.push((path, matches));
            }
        }
        assert_production_excludes(&source.join(root), &["crate::runtime"])?;
    }

    // Then: no hardware-object layer owns actor coordination.
    assert!(leaks.is_empty(), "runtime ownership leaked: {leaks:#?}");
    Ok(())
}

#[test]
fn hardware_pipeline_dependencies_flow_forward() -> io::Result<()> {
    // Given: queue, execution, retirement, and IRQ hardware-object layers.
    let source = source_root();

    // When/Then: each layer is checked against backward dependencies.
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
fn external_device_tests_use_only_public_async_and_abi_surfaces() -> io::Result<()> {
    // Given: every Rust fixture under the device crate's external tests directory.
    let tests = source_root().parent().expect("crate root").join("tests");
    let forbidden = [
        "SyncDevice",
        "ManualRunner",
        "test_support",
        "SemanticDevicePort",
        "pdma_model_device::device::",
        "pdma_model_device::execution::",
        "pdma_model_device::queue::",
        "pdma_model_device::retirement::",
        "include_str!(\"../../src",
        "CARGO_MANIFEST_DIR",
    ];
    let mut leaks = Vec::new();

    // When: test sources are checked for private components and source probes.
    for path in super::support::rust_files(&tests)? {
        let contents = fs::read_to_string(&path)?;
        let matches = forbidden
            .iter()
            .filter(|pattern| contents.contains(**pattern))
            .copied()
            .collect::<Vec<_>>();
        if !matches.is_empty() {
            leaks.push((path, matches));
        }
    }

    // Then: all external tests drive only supported public device boundaries.
    assert!(
        leaks.is_empty(),
        "external test ownership leaked: {leaks:#?}"
    );
    Ok(())
}
