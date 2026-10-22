use crate::harness::runtime::AccessError;
use crate::harness::{self, TestResult};
use pdma_model_contract::control::ControlResponseStatus;
use std::sync::Arc;

#[tokio::test(flavor = "current_thread")]
async fn multi_channel_lifecycle_keeps_peer_copy_and_drains_released_memory() -> TestResult {
    // Given: three hardware channels and one independently executing Host program.
    // When: Host pipelines grants, releases an active copy, and reuses its drained channel.
    let report = harness::multi::run().await?;
    let outcome = report.program?;
    // Then: every response correlates, peer/fresh copies match, and old bytes stop changing.
    assert_eq!(outcome.granted, [0, 1, 2, 0]);
    assert!(outcome.partial_bytes > 0 && outcome.partial_bytes < 65_536);
    assert!(outcome.drained_bytes >= outcome.partial_bytes && outcome.drained_bytes < 65_536);
    assert_eq!(outcome.old_destination[0], outcome.old_destination[1]);
    assert!(
        outcome.old_destination[0]
            .iter()
            .skip(outcome.drained_bytes)
            .all(|byte| *byte == 0)
    );
    assert_eq!(outcome.peer_bytes, b"peer-copy-during-release");
    assert_eq!(outcome.fresh_bytes, b"fresh-driver-after-drain");
    for (request, response) in &outcome.management {
        assert_eq!(response.id, *request);
        assert_eq!(response.status, ControlResponseStatus::Success);
    }
    assert_eq!(outcome.peer, 202);
    assert_eq!(outcome.fresh, 303);
    assert_eq!(outcome.unallocated_access, Err(AccessError::Closed));
    assert_eq!(outcome.released_access, Err(AccessError::Closed));
    assert_eq!(outcome.peer_access, Ok(()));
    assert_eq!(outcome.final_status, [0, 0, 0]);
    assert!(report.summary.deliveries() >= 3);
    assert_eq!(Arc::strong_count(&report.storage), 1);
    println!(
        "lifecycle: grants={:?}, partial_bytes={}, drained_bytes={}, old_memory_stable=true, peer={:?}, fresh={:?}, statuses={:?}, management={:?}, interrupts={}, owners=1",
        outcome.granted,
        outcome.partial_bytes,
        outcome.drained_bytes,
        outcome.peer_bytes,
        outcome.fresh_bytes,
        outcome.final_status,
        outcome.management,
        report.summary.deliveries()
    );
    Ok(())
}
