//! Explicit public-owner compile surface for device integration test support.

include!("test_support_surface/compiler.rs");

#[test]
fn explicit_test_and_runtime_owners_replace_root_forwards() -> Result<(), String> {
    // Given: an independent downstream crate imports each final owner.
    let surface = compile_downstream_surface()?;

    // When: cargo checks the final and stale import paths in isolated target directories.
    let owners_compile = surface.final_owners.succeeded;
    let stale_snapshots_rejected = !surface.stale_root_snapshots.succeeded;
    let stale_runtime_rejected = !surface.stale_root_runtime.succeeded;

    // Then: final owners compile and each retired Device-root ownership path rejects.
    assert!(owners_compile, "{}", surface.final_owners.stderr);
    assert!(stale_snapshots_rejected);
    assert!(stale_runtime_rejected);
    assert!(
        surface
            .stale_root_snapshots
            .mentions_all(["DeviceStateSnapshot", "ResetPartitionSnapshot"])
    );
    assert!(surface.stale_root_runtime.mentions_all([
        "NoopRegisterObserver",
        "RegisterObservation",
        "RegisterObserver",
        "SharedMemory",
    ]));
    println!(
        "final_owners_compile={owners_compile};stale_root_snapshots_rejected={stale_snapshots_rejected};stale_root_runtime_rejected={stale_runtime_rejected}"
    );
    Ok(())
}
