use crate::harness::{self, TestResult};
use pdma_model_contract::control::ControlResponseStatus;

#[tokio::test]
async fn pasid_control_bootstraps_contexts_and_copies_between_identical_addresses() -> TestResult {
    // Given: Default, PASID zero and two payload PASIDs have independent backings.
    // When: software creates contexts, copies through SG and performs lifecycle management.
    let outcome = Box::pin(harness::contexts::run(false)).await?;
    // Then: independent control, list, source and destination bindings all reach their backing.
    assert!(outcome.same_address);
    assert_eq!(outcome.bytes, b"PASID-backed scatter gather copy");
    let copied = outcome.completions.first().ok_or("missing copy response")?;
    assert_eq!(*copied, Ok(37));
    assert_eq!(
        outcome
            .management
            .get(6)
            .ok_or("missing ring-busy result")?
            .status,
        ControlResponseStatus::ContextBusy
    );
    for (index, response) in outcome.management.iter().enumerate() {
        if index != 6 {
            assert_eq!(response.status, ControlResponseStatus::Success);
        }
    }
    assert_eq!(outcome.management.last().unwrap().context, 0);
    for batch in &outcome.mixed {
        assert!(batch.partial > 0 && batch.partial < 65_536);
        assert_eq!(batch.status, ControlResponseStatus::Success);
        assert_eq!(batch.peer, *b"peer");
        assert!(batch.completions.iter().all(Result::is_ok));
    }
    let drained = outcome.mixed.first().ok_or("missing drain batch")?;
    let aborted = outcome.mixed.get(1).ok_or("missing abort batch")?;
    assert_eq!(drained.completions, [Ok(102)]);
    assert_eq!(drained.target, [0x6d; 4]);
    assert_eq!(aborted.completions, [Ok(202)]);
    assert_eq!(aborted.target, [0xa5; 4]);
    Ok(())
}

#[tokio::test]
async fn context_failure_reports_recovery_channel_and_control_survives_reset() -> TestResult {
    // Given: a context-aware copy targets an unmapped source in its own PASID.
    // When: software requests drain after the fault and recovers through ResetChannel.
    let outcome = Box::pin(harness::contexts::run(true)).await?;
    // Then: maintenance reports the stopped channel and the live control service recovers it.
    let failure = outcome
        .completions
        .first()
        .ok_or("missing failed copy")?
        .as_ref()
        .expect_err("copy must fail");
    assert_eq!(failure.code, pdma_model_contract::dma::EventCode::ReadError);
    assert_eq!(failure.bytes_completed(), 0);
    assert_eq!(
        outcome
            .management
            .get(4)
            .ok_or("missing maintenance failure")?
            .status,
        ControlResponseStatus::MaintenanceFailed
    );
    assert_eq!(
        outcome
            .management
            .get(4)
            .ok_or("missing maintenance failure")?
            .channel,
        0
    );
    assert_eq!(outcome.completions.get(1), Some(&Ok(37)));
    assert_eq!(outcome.bytes, b"PASID-backed scatter gather copy");
    assert_eq!(
        outcome.management.last().unwrap().status,
        ControlResponseStatus::Success
    );
    Ok(())
}
