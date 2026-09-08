use super::Fixture;
use pdma_model_contract::{
    abi::{AbiCompletion, AbiEventAccess, AbiEventKind, AbiEventStage, registers::QueueRegisters},
    semantic::ResourceConfig,
};

#[tokio::test(flavor = "current_thread")]
async fn nonzero_dma_addresses_copy_only_the_requested_range() {
    // Given: source and destination extents with distinct sentinel bytes.
    let (fixture, worker) = Fixture::new(ResourceConfig::default()).await;
    fixture
        .handle
        .write_memory(fixture.source, &[10, 11, 12, 13, 14, 15, 16, 17])
        .await
        .expect("source");
    fixture
        .handle
        .write_memory(fixture.destination, &[90; 8])
        .await
        .expect("destination");
    let mut payload = fixture.payload(3);
    payload.source_dma_address += 2;
    payload.destination_dma_address += 3;
    fixture.copy(0, payload).await;
    let worker = tokio::spawn(worker.run());

    // When: the descriptor starts inside both mappings.
    fixture.publish(1).await;
    fixture.device.wait_for_progress_after(0).await;
    fixture.device.request_shutdown().await;
    worker.await.expect("worker joins");

    // Then: only the addressed destination subrange changes.
    assert_eq!(fixture.completion(0).await, AbiCompletion::success(0, 0));
    assert_eq!(
        fixture
            .handle
            .read_memory(fixture.destination, 8)
            .await
            .expect("destination"),
        [90, 90, 90, 12, 13, 14, 90, 90]
    );
}

#[tokio::test(flavor = "current_thread")]
async fn crossing_unmapped_and_overflowing_dma_ranges_publish_no_success() {
    // Given: source ranges that cross a mapping, are unmapped, or overflow.
    let (fixture, worker) = Fixture::new(ResourceConfig::default()).await;
    fixture
        .handle
        .write_memory(fixture.destination, &[90; 8])
        .await
        .expect("destination");
    for (slot, source) in [fixture.source.raw() + 7, 0xffff_0000, u64::MAX]
        .into_iter()
        .enumerate()
    {
        let mut payload = fixture.payload(2);
        payload.source_dma_address = source;
        fixture
            .copy(u64::try_from(slot).expect("slot"), payload)
            .await;
    }
    let worker = tokio::spawn(worker.run());

    // When: the hardware worker fetches all three invalid ranges.
    fixture.publish(3).await;
    fixture.device.wait_for_error_after(2).await;
    fixture.device.request_shutdown().await;
    let summary = worker.await.expect("worker joins");

    // Then: every command produces an error without a CQ record or payload write.
    assert_eq!((summary.steps, summary.errors), (0, 3));
    assert_eq!(
        fixture.cursor(QueueRegisters::CQ_PRODUCER.offset()).await,
        0
    );
    assert_eq!(
        fixture.cursor(QueueRegisters::EQ_PRODUCER.offset()).await,
        3
    );
    for (slot, kind, locator) in [
        (0, AbiEventKind::OutOfBounds, fixture.source.raw() + 8),
        (1, AbiEventKind::UnmappedDmaAddress, 0xffff_0000),
        (2, AbiEventKind::RangeOverflow, u64::MAX),
    ] {
        let event = fixture.event(slot).await;
        assert_eq!(
            (event.command_id, event.kind, event.fault_locator),
            (slot, kind, locator)
        );
        assert_eq!(
            (event.stage, event.access),
            (AbiEventStage::Source, AbiEventAccess::Read)
        );
    }
    assert_eq!(
        fixture
            .handle
            .read_memory(fixture.destination, 8)
            .await
            .expect("destination"),
        [90; 8]
    );
}
