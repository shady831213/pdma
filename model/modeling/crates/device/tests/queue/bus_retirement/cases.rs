use super::support::{Fixture, allocate_pair};
use crate::queue::hardware::{TestResult, write};
use pdma_model_contract::abi::registers::{IrqSourceRegisters, QueueRegisters, irq_offset};
use pdma_model_contract::abi::{AbiCompletion, AbiEvent};
use pdma_model_contract::semantic::{ResourceConfig, TraceEventKind};

#[tokio::test(flavor = "current_thread")]
async fn result_bytes_precede_irq_and_retry_does_not_reexecute() -> TestResult {
    // Given: two DMA copies compete for one completion slot.
    let mut fixture = Fixture::new(ResourceConfig::default()).await?;
    let pair = allocate_pair(&fixture).await?;
    for (offset, value) in [
        (IrqSourceRegisters::ENABLED.offset(), 1),
        (IrqSourceRegisters::MASKED.offset(), 0),
    ] {
        write(&fixture.endpoint, irq_offset(0, 0, offset)?, value).await?;
    }
    fixture.copy(0, pair).await?;
    fixture.copy(1, pair).await?;
    fixture.cursor(QueueRegisters::DOORBELL.offset(), 2).await?;
    let worker = fixture.start()?;
    fixture.handle.wait_for_stall_after(0).await;
    assert_eq!(fixture.read(QueueRegisters::SQ_CONSUMER.offset()).await?, 2);
    assert_eq!(fixture.read(QueueRegisters::CQ_PRODUCER.offset()).await?, 1);
    let first = AbiCompletion::decode(&fixture.handle.read_memory(fixture.rings[1], 32).await?)?;
    assert_eq!(first.command_id(), 10);
    assert!(
        fixture
            .handle
            .irq_snapshot()
            .await
            .is_some_and(pdma_model_contract::semantic::IrqSnapshot::asserted)
    );

    // When: the source changes while retirement waits, then software frees a slot.
    fixture.handle.write_memory(pair[0], &[9; 4]).await?;
    fixture.handle.wait_for_stall_after(1).await;
    fixture
        .cursor(QueueRegisters::CQ_CONSUMER.offset(), 1)
        .await?;
    fixture.handle.wait_for_progress_after(1).await;

    // Then: retirement retries, preserving executed bytes and trace ordering.
    assert_eq!(fixture.handle.read_memory(pair[1], 4).await?, [1, 3, 5, 7]);
    let second = AbiCompletion::decode(&fixture.handle.read_memory(fixture.rings[1], 32).await?)?;
    assert_eq!(second.command_id(), 11);
    assert_eq!(fixture.read(QueueRegisters::CQ_PRODUCER.offset()).await?, 2);
    let trace = fixture.handle.trace_events().await;
    for kind in [
        TraceEventKind::ExecutionSucceeded,
        TraceEventKind::CompletionPublished,
        TraceEventKind::IrqSourceLatched,
    ] {
        assert_eq!(trace.iter().filter(|event| event.kind() == kind).count(), 2);
    }
    let published = trace
        .iter()
        .position(|event| event.kind() == TraceEventKind::CompletionPublished)
        .ok_or("publication trace")?;
    let latched = trace
        .iter()
        .position(|event| event.kind() == TraceEventKind::IrqSourceLatched)
        .ok_or("IRQ trace")?;
    assert!(published < latched);
    fixture.handle.request_shutdown().await;
    worker.await?;
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn event_pressure_retries_one_fault_after_consumer_advance() -> TestResult {
    // Given: two malformed descriptors and one event slot.
    let mut fixture = Fixture::new(ResourceConfig::default()).await?;
    fixture.bytes(0, &[0; 64]).await?;
    fixture.bytes(1, &[0; 64]).await?;
    fixture.cursor(QueueRegisters::DOORBELL.offset(), 2).await?;
    let worker = fixture.start()?;
    fixture.handle.wait_for_stall_after(0).await;
    assert_eq!(fixture.read(QueueRegisters::EQ_PRODUCER.offset()).await?, 1);
    assert_eq!(fixture.read(QueueRegisters::SQ_CONSUMER.offset()).await?, 2);
    let first = AbiEvent::decode(&fixture.handle.read_memory(fixture.rings[2], 64).await?)?;

    // When: software releases event space.
    fixture
        .cursor(QueueRegisters::EQ_CONSUMER.offset(), 1)
        .await?;
    fixture.handle.wait_for_error_after(1).await;

    // Then: the second fault retires once, with no successful completion.
    let second = AbiEvent::decode(&fixture.handle.read_memory(fixture.rings[2], 64).await?)?;
    assert_eq!(first.fault().tuple().1, fixture.rings[0].raw());
    assert_eq!(second.fault().tuple().1, fixture.rings[0].raw() + 64);
    assert_eq!(fixture.read(QueueRegisters::EQ_PRODUCER.offset()).await?, 2);
    assert_eq!(fixture.read(QueueRegisters::CQ_PRODUCER.offset()).await?, 0);
    assert_eq!(fixture.handle.native_steps().await, 2);
    fixture.handle.request_shutdown().await;
    worker.await?;
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn corrupt_result_and_masked_line_preserve_consumer_and_pending_cause() -> TestResult {
    // Given: one completion whose IRQ source is enabled but remains masked.
    let mut fixture = Fixture::new(ResourceConfig::default()).await?;
    let pair = allocate_pair(&fixture).await?;
    write(
        &fixture.endpoint,
        irq_offset(0, 0, IrqSourceRegisters::ENABLED.offset())?,
        1,
    )
    .await?;
    fixture.copy(0, pair).await?;
    fixture.cursor(QueueRegisters::DOORBELL.offset(), 1).await?;
    let worker = fixture.start()?;
    fixture.handle.wait_for_progress_after(0).await;
    assert!(
        fixture
            .handle
            .irq_snapshot()
            .await
            .is_some_and(|irq| !irq.asserted())
    );

    // When: corrupt bytes fail decoding and software acknowledges the occupied ring.
    fixture
        .handle
        .write_memory(fixture.rings[1], &[0xff])
        .await?;
    assert!(
        AbiCompletion::decode(&fixture.handle.read_memory(fixture.rings[1], 32).await?).is_err()
    );
    write(
        &fixture.endpoint,
        irq_offset(0, 0, IrqSourceRegisters::ACK.offset())?,
        1,
    )
    .await?;
    write(
        &fixture.endpoint,
        irq_offset(0, 0, IrqSourceRegisters::MASKED.offset())?,
        0,
    )
    .await?;

    // Then: the unchanged consumer retains the cause that asserts the line.
    assert!(fixture.handle.wait_irq_notification_ready().await);
    assert_eq!(fixture.read(QueueRegisters::CQ_CONSUMER.offset()).await?, 0);
    assert!(
        fixture
            .handle
            .irq_snapshot()
            .await
            .is_some_and(pdma_model_contract::semantic::IrqSnapshot::asserted)
    );
    fixture.handle.request_shutdown().await;
    worker.await?;
    Ok(())
}
