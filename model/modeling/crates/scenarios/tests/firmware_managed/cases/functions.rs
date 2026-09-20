use crate::harness::{self, TestResult};
use pdma_model_contract::ResponseStatus;

#[tokio::test]
async fn identical_dma_addresses_resolve_in_the_owning_functions_memory() -> TestResult {
    // Given: PF and both VFs have independent memory with the same allocation base.
    let payloads: [&[u8]; 2] = [b"AAAA-function-one", b"BBBB-function-two"];
    // When: firmware grants channels and both VFs submit IRQ-driven copies.
    let outcomes = harness::functions::run_isolated(payloads).await?;
    // Then: descriptors, payloads and responses stay in each VF's address space.
    for (outcome, payload) in outcomes.iter().zip(payloads) {
        assert_eq!(outcome.bytes, payload);
        assert_eq!(outcome.response.status, ResponseStatus::Success);
        assert_eq!(
            outcome.response.bytes_completed,
            u64::try_from(payload.len())?
        );
        assert_eq!(outcome.interrupts, 1);
    }
    Ok(())
}

#[tokio::test]
async fn resetting_one_functions_active_copy_preserves_peer_dma() -> TestResult {
    // Given: A has a partially copied command and B submits independent work.
    // When: A requests channel reset through firmware while both tasks are outstanding.
    let (reset, (peer, bytes)) = harness::functions::run_reset().await?;
    // Then: old A writes stop at completion, B succeeds, and fresh A work succeeds.
    assert!(reset.before_reset > 0 && reset.before_reset < 65_536);
    assert!(reset.at_reset.contains(&0));
    assert_eq!(reset.at_reset, reset.after_peer);
    assert_eq!(peer.status, ResponseStatus::Success);
    assert_eq!(peer.id, 200);
    assert_eq!(bytes, b"peer-function-copy");
    assert_eq!(reset.fresh.status, ResponseStatus::Success);
    assert_eq!(reset.fresh.id, 101);
    assert_eq!(reset.fresh_bytes, b"after-function-channel-reset");
    Ok(())
}

#[tokio::test]
async fn two_functions_copy_through_shared_dma_and_receive_only_their_own_irq() -> TestResult {
    // Given: two software programs with disjoint control ingress and IRQ destinations.
    let payloads: [&[u8]; 2] = [b"Function-A-data", b"Function-B-distinct-payload"];
    // When: both execute acquire, IRQ-driven DMA, reset and release on the same DMA.
    let outcomes = harness::functions::run(payloads).await?;
    // Then: both copies complete through their own ISR without cross-delivery.
    assert_ne!(outcomes[0].channel, outcomes[1].channel);
    for (outcome, payload) in outcomes.iter().zip(payloads) {
        assert_eq!(outcome.bytes, payload);
        assert_eq!(outcome.response.status, ResponseStatus::Success);
        assert_eq!(
            outcome.response.bytes_completed,
            u64::try_from(payload.len())?
        );
        assert_eq!(outcome.interrupts, 1);
    }
    Ok(())
}
