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

#[test]
fn exact_frontend_builds_private_execution_commands_without_external_reconstruction() {
    // Given: the exact descriptor fetch and operation-resolution boundary.
    let exact = [
        include_str!("../src/runner/native/io/exact/submission.rs"),
        include_str!("../src/runner/native/io/exact/submission/command.rs"),
        include_str!("../src/runner/native/io/exact/submission/command/copy.rs"),
        include_str!("../src/runner/native/io/exact/submission/command/broadcast.rs"),
        include_str!("../src/runner/native/io/exact/submission/command/vector.rs"),
    ];
    let public_types = [
        "semantic::desc",
        "ExternalCommand",
        "CopyCommand",
        "StridedCopyCommand",
        "BroadcastCopyCommand",
        "VectorAddCommand",
        "CopyRequest",
        "StridedCopyRequest",
        "BroadcastCopyRequest",
        "VectorAddRequest",
        "BufferId::new(0)",
    ];

    // When: exact resolution source is inspected for semantic reconstruction.
    let leaked = exact
        .iter()
        .flat_map(|source| public_types.iter().filter(|name| source.contains(*name)))
        .copied()
        .collect::<Vec<_>>();

    // Then: exact decode produces the private execution representation directly.
    assert!(
        leaked.is_empty(),
        "semantic types in exact resolver: {leaked:?}"
    );
}
