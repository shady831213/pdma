//! Guards the device-private execution-command boundary.

#[test]
fn native_execution_core_is_external_command_free() {
    // Given: the device files below the semantic and exact frontend boundary.
    let core = [
        include_str!("../src/runner/native/step.rs"),
        include_str!("../src/runner/native/resources.rs"),
        include_str!("../src/runner/native/copy.rs"),
        include_str!("../src/runner/native/broadcast.rs"),
        include_str!("../src/runner/native/vector_add.rs"),
        include_str!("../src/trace.rs"),
        include_str!("../src/trace/event.rs"),
    ];
    let public_types = [
        "ExternalCommand",
        "CopyCommand",
        "StridedCopyCommand",
        "BroadcastCopyCommand",
        "VectorAddCommand",
    ];

    // When: the core source is inspected for host-facing command vocabulary.
    let leaked = core
        .iter()
        .flat_map(|source| public_types.iter().filter(|name| source.contains(*name)))
        .copied()
        .collect::<Vec<_>>();

    // Then: only private execution vocabulary crosses into scheduling and execution.
    assert!(
        leaked.is_empty(),
        "host-facing types in device core: {leaked:?}"
    );
}
