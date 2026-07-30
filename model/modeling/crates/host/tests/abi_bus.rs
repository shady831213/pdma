//! Public raw PDMA bus behavioral contract.

use pdma_model_contract::abi::{
    DeviceEpoch, DmaAddress, DmaRegion, EpochQueue, QueueActivation, QueueCursorUpdate,
    QueueRingRegions, RingCursor, RingCursors, RingLayout, RingRecord, WholeDeviceReset,
};
use pdma_model_contract::{
    BufferId, EventRouting, IrqMessage, IrqMessageId, IrqNotification, IrqVector, ModelError,
    ModelResult, QueueConfig, QueueId, SoftwareOwnerId,
};
use pdma_model_device::{AsyncDevice, AsyncDeviceHandle, BusControlError, DmaSpaceError};
use pdma_model_host::{
    AsyncPdmaBus, BusControlErrorAdapter, BusDmaErrorAdapter, BusDmaStage, BusDmaWrite,
    BusQueueStatus, BusSubmission, QueueControlWrite,
};
use std::fmt;
use std::sync::{Arc, Mutex};

include!("abi_bus/support.rs");
include!("abi_bus/failure.rs");

#[tokio::test]
async fn raw_bus_returns_regions_and_records_exact_atomic_dma_control_irq_sequence()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: a consumer-defined bus and one complete raw queue transaction program.
    let bus = FakeBus::new();
    let allocated = bus.allocate_dma_region(32).await?;
    let rings = bus.stage_dma_regions(&[128, 64, 128]).await?;
    let proposed = rings.proposed_regions().to_vec();
    let [submission, _, _] = proposed.as_slice() else {
        return Err("recording bus must propose three ring regions".into());
    };
    let activation = activation(&proposed, QueueId::new(0))?;

    // When: the consumer drives allocation, atomic bootstrap, DMA, control, acceptance, and IRQ.
    bus.bootstrap_queue(rings, activation).await?;
    bus.write_dma(allocated.base(), &[1, 3, 5, 7]).await?;
    let bytes = bus.read_dma(allocated.base(), 4).await?;
    let queue = EpochQueue::new(DeviceEpoch::initial(), QueueId::new(0));
    bus.write_queue_control(QueueControlWrite::CaptureSubmissionProducer(
        QueueCursorUpdate::new(queue, RingCursor::new(0)),
    ))
    .await?;
    let status = bus.read_queue_status(queue).await?;
    let indirect = bus.stage_dma_regions(&[8]).await?;
    let [indirect_region] = indirect.proposed_regions() else {
        return Err("recording bus must propose one indirect region".into());
    };
    let indirect_region = *indirect_region;
    bus.accept_submission(BusSubmission::new(
        queue,
        RingCursor::new(0),
        BusDmaWrite::new(submission.base(), vec![0x5a; 64]),
        Some((
            indirect,
            BusDmaWrite::new(indirect_region.base(), vec![0xa5; 8]),
        )),
    ))
    .await?;
    let notification = bus
        .wait_irq_notification()
        .await
        .ok_or("recording IRQ endpoint closed")?;

    // Then: regions are the sole object/IOVA handoff and the exact raw sequence is observable.
    assert_eq!(allocated.object().raw(), 0);
    assert_eq!(allocated.base(), DmaAddress::new(0x1_0000));
    assert_eq!(bytes, vec![1, 3, 5, 7]);
    assert_eq!(status.queue(), queue);
    assert_eq!(notification.epoch(), DeviceEpoch::initial());
    assert!(matches!(notification, IrqNotification::Message(_)));
    assert_eq!(
        bus.sequence(),
        expected_sequence(allocated, &proposed, activation)?
    );
    real_bus_happy_probe().await?;
    Ok(())
}

#[tokio::test]
async fn pending_dropped_conflicting_failed_and_stale_bus_operations_do_not_commit()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: a real raw bus with dropped futures, conflicting stages, and malformed requests.
    let result = real_bus_failure_probe().await;

    // When: every pending, conflicting, failed, and stale operation has been exercised.
    // Then: the probe returns only after observing unchanged mappings, bytes, cursors, and epoch.
    result
}
