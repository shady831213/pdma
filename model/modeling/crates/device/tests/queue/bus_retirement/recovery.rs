use super::support::{Fixture, allocate_pair};
use crate::queue::hardware::{self, TestResult, configure, read, write};
use pdma_model_contract::abi::registers::{
    GlobalRegisters, IrqSourceRegisters, QueueRegisters, irq_offset, queue_offset,
};
use pdma_model_contract::abi::{
    AbiCommand, AbiCommandPayloadValues, AbiCommandPayloadVectorAddValues, DmaAddress,
};
use pdma_model_contract::semantic::{DeviceEpoch, ResourceConfig, TraceEventKind};

#[tokio::test(flavor = "current_thread")]
async fn reset_discards_blocked_retirement_and_preserves_dma() -> TestResult {
    // Given: a second execution is waiting for completion space.
    let mut fixture = Fixture::new(ResourceConfig::default()).await?;
    let pair = allocate_pair(&fixture).await?;
    fixture.copy(0, pair).await?;
    fixture.copy(1, pair).await?;
    fixture.cursor(QueueRegisters::DOORBELL.offset(), 2).await?;
    let worker = fixture.start()?;
    fixture.handle.wait_for_stall_after(0).await;
    // When: reset discards retained retirement and software activates fresh rings.
    write(
        &fixture.endpoint,
        u32::try_from(GlobalRegisters::RESET.offset())?,
        1,
    )
    .await?;
    let fresh = fixture.endpoint.at_epoch(DeviceEpoch::new(1));
    configure(
        &fresh,
        0,
        [
            (fixture.rings[0], 4),
            (fixture.rings[1], 1),
            (fixture.rings[2], 1),
        ],
    )
    .await?;
    write(&fresh, queue_offset(0, QueueRegisters::ENABLE.offset())?, 1).await?;
    hardware::idle(&fixture.handle).await;
    // Then: DMA persists while no discarded completion reaches the new queue.
    assert_eq!(fixture.handle.read_memory(pair[1], 4).await?, [1, 3, 5, 7]);
    assert_eq!(
        read(
            &fresh,
            queue_offset(0, QueueRegisters::CQ_PRODUCER.offset())?
        )
        .await?,
        0
    );
    assert_eq!(
        read(
            &fresh,
            queue_offset(0, QueueRegisters::SQ_CONSUMER.offset())?
        )
        .await?,
        0
    );
    assert_eq!(
        fixture
            .handle
            .trace_events()
            .await
            .iter()
            .filter(|event| event.kind() == TraceEventKind::CompletionPublished)
            .count(),
        1
    );
    assert_eq!(fixture.handle.irq_snapshot().await, None);
    fixture.handle.request_shutdown().await;
    worker.await?;
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn reset_stales_a_delivered_message_before_ack_mutation() -> TestResult {
    // Given: a completion source emits one host-owned message.
    let mut fixture = Fixture::new(ResourceConfig::default()).await?;
    let pair = allocate_pair(&fixture).await?;
    for (offset, value) in [
        (IrqSourceRegisters::ROUTE_KIND.offset(), 1),
        (IrqSourceRegisters::ROUTE_VECTOR.offset(), 9),
        (IrqSourceRegisters::ENABLED.offset(), 1),
        (IrqSourceRegisters::MASKED.offset(), 0),
    ] {
        write(&fixture.endpoint, irq_offset(0, 0, offset)?, value).await?;
    }
    fixture.copy(0, pair).await?;
    fixture.cursor(QueueRegisters::DOORBELL.offset(), 1).await?;
    let worker = fixture.start()?;
    assert!(
        fixture
            .handle
            .wait_irq_notification_ready_at_epoch(DeviceEpoch::initial())
            .await?
    );
    let notification = fixture
        .handle
        .take_irq_notification_at_epoch(DeviceEpoch::initial())
        .await?
        .ok_or("message IRQ")?;
    assert!(
        matches!(notification, pdma_model_contract::semantic::IrqNotification::Message(message) if message.id().raw() == 0)
    );
    // When: reset invalidates the message epoch and its bound register endpoint.
    write(
        &fixture.endpoint,
        u32::try_from(GlobalRegisters::RESET.offset())?,
        1,
    )
    .await?;
    let before = fixture.handle.snapshot().await;
    // Then: stale acknowledgement and notification access reject without mutation.
    assert!(
        write(
            &fixture.endpoint,
            irq_offset(0, 0, IrqSourceRegisters::ACK.offset())?,
            1
        )
        .await
        .is_err()
    );
    assert!(
        fixture
            .handle
            .take_irq_notification_at_epoch(notification.source_capability().epoch())
            .await
            .is_err()
    );
    assert_eq!(fixture.handle.snapshot().await, before);
    fixture.handle.request_shutdown().await;
    worker.await?;
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn resource_starved_queue_does_not_block_another_hardware_queue() -> TestResult {
    // Given: a vector queue has no compute credits and another queue has a valid copy.
    let mut fixture = Fixture::new(ResourceConfig::default().with_compute_slots(0)).await?;
    let pair = allocate_pair(&fixture).await?;
    let second_destination = hardware::allocate(&fixture.memory, 4)?.base();
    let vector = AbiCommand::from_payload(
        40,
        AbiCommandPayloadValues::VectorAdd(AbiCommandPayloadVectorAddValues {
            source_a_dma_address: pair[0].raw(),
            source_b_dma_address: pair[0].raw(),
            destination_dma_address: second_destination.raw(),
            length: 4.into(),
        }),
    );
    let mut bytes = [0; 64];
    vector.encode(&mut bytes)?;
    fixture.bytes(0, &bytes).await?;
    fixture.cursor(QueueRegisters::DOORBELL.offset(), 1).await?;
    let [second_sq, _, _] = hardware::activate(&fixture.handle, &fixture.memory, 1, [2; 3]).await?;
    let copy = AbiCommand::from_payload(
        41,
        AbiCommandPayloadValues::Copy(pdma_model_contract::abi::AbiCommandPayloadCopyValues {
            source_dma_address: pair[0].raw(),
            destination_dma_address: pair[1].raw(),
            length: 4.into(),
        }),
    );
    copy.encode(&mut bytes)?;
    fixture.handle.write_memory(second_sq, &bytes).await?;
    for offset in [
        QueueRegisters::SQ_PRODUCER.offset(),
        QueueRegisters::DOORBELL.offset(),
    ] {
        write(&fixture.endpoint, queue_offset(1, offset)?, 1).await?;
    }
    // When: the worker arbitrates both captured hardware queues.
    let worker = fixture.start()?;
    fixture.handle.wait_for_stall_after(0).await;
    fixture.handle.wait_for_progress_after(0).await;
    // Then: only the runnable copy executes and the vector destination stays untouched.
    assert_eq!(fixture.read(QueueRegisters::SQ_CONSUMER.offset()).await?, 0);
    assert_eq!(
        read(
            &fixture.endpoint,
            queue_offset(1, QueueRegisters::SQ_CONSUMER.offset())?
        )
        .await?,
        1
    );
    assert_eq!(fixture.handle.read_memory(pair[1], 4).await?, [1, 3, 5, 7]);
    assert_eq!(
        fixture.handle.read_memory(second_destination, 4).await?,
        [0; 4]
    );
    let scheduled: Vec<_> = fixture
        .handle
        .trace_events()
        .await
        .into_iter()
        .filter(|event| event.kind() == TraceEventKind::Scheduled)
        .filter_map(|event| event.queue_id())
        .collect();
    assert_eq!(scheduled, [pdma_model_contract::semantic::QueueId::new(1)]);
    fixture.handle.request_shutdown().await;
    worker.await?;
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn execution_memory_fault_retires_once_without_success() -> TestResult {
    // Given: a copy descriptor has an overflowing destination.
    let mut fixture = Fixture::new(ResourceConfig::default()).await?;
    let pair = allocate_pair(&fixture).await?;
    fixture
        .copy(0, [pair[0], DmaAddress::new(u64::MAX)])
        .await?;
    fixture.cursor(QueueRegisters::DOORBELL.offset(), 1).await?;
    // When: the worker executes the captured descriptor.
    let worker = fixture.start()?;
    fixture.handle.wait_for_error_after(0).await;
    // Then: one event is published with no completion or reexecution.
    assert_eq!(fixture.read(QueueRegisters::EQ_PRODUCER.offset()).await?, 1);
    assert_eq!(fixture.read(QueueRegisters::CQ_PRODUCER.offset()).await?, 0);
    assert_eq!(fixture.handle.native_steps().await, 1);
    assert_eq!(
        fixture
            .handle
            .trace_events()
            .await
            .iter()
            .filter(|event| event.kind() == TraceEventKind::EventPublished)
            .count(),
        1
    );
    fixture.handle.request_shutdown().await;
    worker.await?;
    Ok(())
}
