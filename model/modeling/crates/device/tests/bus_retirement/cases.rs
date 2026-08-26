use super::TestResult;
use super::support::{Fixture, allocate_pair};
use pdma_model_contract::abi::RingCursor;
use pdma_model_contract::semantic::{
    CommandId, IrqMessageId, IrqNotification, IrqRoute, IrqVector, TraceEventKind,
};
use pdma_model_contract::semantic::{DeviceEpoch, WholeDeviceReset};
use pdma_model_contract::translation::{decode_completion, decode_event};
use pdma_model_device::PendingDmaResult;

/// Exercises exact event visibility and retry ordering.
///
/// # Panics
///
/// Panics when retirement visibility or trace ordering differs from the pinned contract.
pub async fn visible_before_irq_and_retry() -> TestResult {
    // Given: two DMA copies and one completion slot behind an enabled Line source.
    let mut fixture = Fixture::new(DeviceEpoch::initial(), 1).await?;
    let (source, first) = allocate_pair(&fixture.handle).await?;
    let (_, second) = allocate_pair(&fixture.handle).await?;
    fixture.handle.enable_tracing().await;
    let irq = fixture.completion_source();
    fixture.handle.enable_irq_source(irq).await?;
    fixture.handle.unmask_irq_source(irq).await?;
    fixture
        .submit_copy(0, 10, source.base(), first.base())
        .await?;
    fixture
        .submit_copy(1, 11, source.base(), second.base())
        .await?;
    fixture.capture(2).await?;
    let worker = fixture.start()?;

    // When: the first result publishes, the second stalls, and consumer advance wakes retry.
    fixture.handle.wait_for_stall_after(0).await;
    let before_retry = fixture.status().await?;
    let first_bytes = fixture.handle.read_memory(fixture.completion, 32).await?;
    let first_record = decode_completion(&first_bytes)?;
    let trace_before_retry = fixture.handle.trace_events().await;
    fixture.consume_completion(1).await?;
    fixture.handle.wait_for_progress_after(1).await;
    let after_retry = fixture.status().await?;
    let second_bytes = fixture.handle.read_memory(fixture.completion, 32).await?;
    let second_record = decode_completion(&second_bytes)?;
    let trace_after_retry = fixture.handle.trace_events().await;

    // Then: bytes precede producer/trace/IRQ and retry publishes the cached result once.
    assert_eq!(before_retry.completion().producer(), RingCursor::new(1));
    assert_eq!(before_retry.submission().consumer(), RingCursor::new(2));
    assert_eq!(first_record.command_id(), CommandId::new(10));
    assert_eq!(after_retry.completion().producer(), RingCursor::new(2));
    assert_eq!(second_record.command_id(), CommandId::new(11));
    assert_eq!(
        trace_after_retry
            .iter()
            .filter(|event| event.kind() == TraceEventKind::ExecutionSucceeded)
            .count(),
        2
    );
    assert_eq!(
        trace_after_retry
            .iter()
            .filter(|event| event.kind() == TraceEventKind::CompletionPublished)
            .count(),
        2
    );
    let publication = trace_before_retry
        .iter()
        .position(|event| event.kind() == TraceEventKind::CompletionPublished)
        .ok_or("completion publication trace")?;
    let latch = trace_before_retry
        .iter()
        .position(|event| event.kind() == TraceEventKind::IrqSourceLatched)
        .ok_or("IRQ latch trace")?;
    assert!(publication < latch);
    assert!(fixture.handle.irq_snapshot().await.is_some());
    fixture.handle.request_shutdown().await;
    worker.await?;

    let mut event_fixture = Fixture::new(DeviceEpoch::initial(), 1).await?;
    event_fixture.handle.enable_tracing().await;
    event_fixture.submit_bytes(0, [0; 64]).await?;
    event_fixture.capture(1).await?;
    let event_worker = event_fixture.start()?;
    event_fixture.handle.wait_for_progress_after(1).await;
    let first_event = event_fixture
        .handle
        .read_memory(event_fixture.event, 64)
        .await?;
    assert!(decode_event(&first_event).is_ok());
    event_fixture.submit_bytes(1, [0; 64]).await?;
    event_fixture.capture(2).await?;
    event_fixture.handle.wait_for_stall_after(0).await;
    let event_status = event_fixture.status().await?;
    assert_eq!(event_status.event().producer(), RingCursor::new(1));
    assert_eq!(event_status.submission().consumer(), RingCursor::new(2));
    assert!(matches!(
        event_fixture.handle.pending_dma_result().await,
        Some(PendingDmaResult::Event(_))
    ));
    assert_eq!(
        event_fixture
            .handle
            .trace_events()
            .await
            .iter()
            .filter(|event| event.kind() == TraceEventKind::EventPublished)
            .count(),
        1
    );
    event_fixture.handle.request_shutdown().await;
    event_worker.await?;
    Ok(())
}

/// Exercises one cached completion under publication pressure.
///
/// # Panics
///
/// Panics when pressure causes reexecution, cursor drift, or early IRQ delivery.
pub async fn publication_pressure_keeps_one_pending() -> TestResult {
    // Given: a filled result ring behind an enabled but masked Line source.
    let mut fixture = Fixture::new(DeviceEpoch::initial(), 1).await?;
    let (source, first) = allocate_pair(&fixture.handle).await?;
    let (_, second) = allocate_pair(&fixture.handle).await?;
    let irq = fixture.completion_source();
    fixture.handle.enable_tracing().await;
    fixture.handle.enable_irq_source(irq).await?;
    fixture
        .submit_copy(0, 20, source.base(), first.base())
        .await?;
    fixture.capture(1).await?;
    let worker = fixture.start()?;
    fixture.handle.wait_for_progress_after(0).await;
    fixture
        .submit_copy(1, 21, source.base(), second.base())
        .await?;
    fixture.capture(2).await?;

    // When: publication cannot claim the occupied slot.
    fixture.handle.wait_for_stall_after(0).await;
    let status = fixture.status().await?;
    let pending = fixture.handle.pending_dma_result().await;

    // Then: exactly one executed result is cached, producer is unchanged, and no early IRQ fires.
    assert_eq!(status.completion().producer(), RingCursor::new(1));
    assert_eq!(status.submission().consumer(), RingCursor::new(2));
    assert!(matches!(
        pending,
        Some(PendingDmaResult::Completion(record))
            if record.command_id() == CommandId::new(21)
    ));
    assert_eq!(
        fixture
            .handle
            .read_memory(second.base(), second.length())
            .await?,
        [1, 3, 5, 7]
    );
    assert!(
        fixture
            .handle
            .irq_snapshot()
            .await
            .is_some_and(|snapshot| !snapshot.asserted())
    );
    assert_eq!(
        fixture
            .handle
            .trace_events()
            .await
            .iter()
            .filter(|event| event.kind() == TraceEventKind::IrqSourceLatched)
            .count(),
        1
    );
    fixture.handle.request_shutdown().await;
    worker.await?;
    Ok(())
}

/// Exercises malformed, masked, and stale exact retirement state.
///
/// # Panics
///
/// Panics when malformed or stale state mutates the pinned device state.
pub async fn corrupt_masked_and_stale() -> TestResult {
    // Given: one masked Line result published into host DMA.
    let mut fixture = Fixture::new(DeviceEpoch::initial(), 1).await?;
    let (source, destination) = allocate_pair(&fixture.handle).await?;
    let irq = fixture.completion_source();
    fixture.handle.enable_irq_source(irq).await?;
    fixture
        .submit_copy(0, 30, source.base(), destination.base())
        .await?;
    fixture.capture(1).await?;
    let worker = fixture.start()?;
    fixture.handle.wait_for_progress_after(0).await;

    // When: host bytes are corrupt, software leaves consumer unchanged and exercises Line ack.
    fixture
        .handle
        .write_memory(fixture.completion, &[0xff])
        .await?;
    let corrupt = fixture.handle.read_memory(fixture.completion, 32).await?;
    assert!(decode_completion(&corrupt).is_err());
    assert_eq!(
        fixture.status().await?.completion().consumer(),
        RingCursor::new(0)
    );
    fixture.handle.ack_irq_source(irq).await?;
    fixture.handle.unmask_irq_source(irq).await?;

    // Then: the queued DMA record retains the Line cause, while reset stales a host-owned message.
    assert!(
        fixture
            .handle
            .irq_snapshot()
            .await
            .is_some_and(pdma_model_contract::semantic::IrqSnapshot::asserted)
    );
    fixture.handle.request_shutdown().await;
    worker.await?;

    let mut message_fixture = Fixture::new(DeviceEpoch::initial(), 1).await?;
    let (message_source, message_destination) = allocate_pair(&message_fixture.handle).await?;
    let message_irq = message_fixture.completion_source();
    let vector = IrqVector::new(9);
    message_fixture
        .handle
        .configure_irq_route(message_irq, IrqRoute::Message(vector))
        .await?;
    message_fixture
        .handle
        .enable_irq_source(message_irq)
        .await?;
    message_fixture
        .handle
        .unmask_irq_source(message_irq)
        .await?;
    message_fixture
        .submit_copy(0, 31, message_source.base(), message_destination.base())
        .await?;
    message_fixture.capture(1).await?;
    let worker = message_fixture.start()?;
    let notification = message_fixture
        .handle
        .wait_irq_notification()
        .await
        .ok_or("message notification")?;
    assert!(matches!(
        notification,
        IrqNotification::Message(message)
            if message.id() == IrqMessageId::new(0) && message.source() == message_irq.source()
    ));
    message_fixture
        .handle
        .reset(WholeDeviceReset::new(DeviceEpoch::initial()))
        .await?;
    assert!(
        message_fixture
            .handle
            .ack_irq_source(notification.source_capability())
            .await
            .is_err()
    );
    message_fixture.handle.request_shutdown().await;
    worker.await?;
    Ok(())
}
