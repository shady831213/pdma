use super::support::Fixture;
use hardware_interface_runtime::RegisterAccess;
use pdma_model_contract::{
    abi::registers::{IrqSourceRegisters, QueueRegisters, irq_offset},
    semantic::ResourceConfig,
};

mod stale;

#[tokio::test(flavor = "current_thread")]
async fn completion_irq_latches_only_after_publication_retry_succeeds() {
    // Given: two copies sharing one completion credit and a line IRQ.
    let (fixture, worker) =
        Fixture::new(ResourceConfig::default().with_completion_publication_slots(1)).await;
    fixture.enable_irq(0).await;
    for slot in 0..2 {
        fixture.copy(slot, fixture.payload(4)).await;
    }
    let worker = tokio::spawn(worker.run());

    // When: an early ack precedes freeing the credit for the retained result.
    fixture.publish(2).await;
    let first = fixture.device.wait_for_progress_after(0).await;
    tokio::task::yield_now().await;
    let stalled = fixture.device.snapshot().await;
    let first_irq = fixture.handle.irq_snapshot().await.expect("first IRQ");
    fixture
        .endpoint
        .write(
            irq_offset(0, 0, IrqSourceRegisters::ACK.offset()).expect("ack"),
            &1_u32.to_le_bytes(),
        )
        .await
        .expect("ack");
    let retained = fixture.handle.irq_snapshot().await;
    fixture
        .write_queue(QueueRegisters::CQ_CONSUMER.offset(), &1_u64.to_le_bytes())
        .await;
    let retry = fixture.device.wait_for_progress_after(1).await;
    let retry_irq = fixture.handle.irq_snapshot().await.expect("retry IRQ");
    fixture.device.request_shutdown().await;
    let summary = worker.await.expect("join");

    // Then: record-backed assertion survives early ack and retry counts once.
    assert_eq!((first.steps, retry.steps, summary.steps), (1, 2, 2));
    assert!(stalled.stalls >= 1);
    assert!(first_irq.asserted());
    assert!(retained.is_some_and(pdma_model_contract::semantic::IrqSnapshot::asserted));
    assert!(retry_irq.asserted());
    assert_eq!(retry_irq.cause().source(), first_irq.cause().source());
}

#[tokio::test(flavor = "current_thread")]
async fn event_irq_latches_only_after_publication_retry_succeeds() {
    // Given: two invalid copies sharing one event publication credit.
    let (fixture, worker) =
        Fixture::new(ResourceConfig::default().with_event_publication_slots(1)).await;
    fixture.enable_irq(1).await;
    for slot in 0..2 {
        fixture.copy(slot, fixture.payload(9)).await;
    }
    let worker = tokio::spawn(worker.run());

    // When: software acknowledges before releasing event credit.
    fixture.publish(2).await;
    let first = fixture.device.wait_for_error_after(0).await;
    tokio::task::yield_now().await;
    let stalled = fixture.device.snapshot().await;
    let first_irq = fixture.handle.wait_irq().await.expect("event IRQ");
    fixture
        .endpoint
        .write(
            irq_offset(0, 1, IrqSourceRegisters::ACK.offset()).expect("ack"),
            &1_u32.to_le_bytes(),
        )
        .await
        .expect("ack");
    let retained = fixture.handle.irq_snapshot().await;
    fixture
        .write_queue(QueueRegisters::EQ_CONSUMER.offset(), &1_u64.to_le_bytes())
        .await;
    let retry = fixture.device.wait_for_error_after(1).await;
    let retry_irq = fixture.handle.wait_irq().await.expect("retry IRQ");
    fixture.device.request_shutdown().await;
    let summary = worker.await.expect("join");

    // Then: both faults publish, preserve assertion and never publish success.
    assert_eq!((first.errors, retry.errors, summary.errors), (1, 2, 2));
    assert!(stalled.stalls >= 1);
    assert!(first_irq.asserted());
    assert!(retained.is_some_and(pdma_model_contract::semantic::IrqSnapshot::asserted));
    assert!(retry_irq.asserted());
    assert_eq!(
        fixture.cursor(QueueRegisters::CQ_PRODUCER.offset()).await,
        0
    );
}

#[tokio::test(flavor = "current_thread")]
async fn absent_irq_source_write_fails_without_creating_a_notification() {
    // Given: only queue zero exists.
    let (fixture, _worker) = Fixture::new(ResourceConfig::default()).await;

    // When: a direct write addresses an absent queue's IRQ source.
    let result = fixture
        .endpoint
        .write(
            irq_offset(99, 0, IrqSourceRegisters::ENABLED.offset()).expect("address"),
            &1_u32.to_le_bytes(),
        )
        .await;

    // Then: the rejected access creates no notification.
    assert!(result.is_err());
    assert!(!fixture.handle.irq_notification_ready().await);
}
