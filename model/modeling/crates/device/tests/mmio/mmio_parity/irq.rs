use super::super::mmio_irq::support::i;
use super::*;
use pdma_model_contract::abi::registers::IrqSourceRegisters;
use pdma_model_contract::semantic::{DeviceEpoch, irq::IrqNotification};

#[tokio::test(flavor = "current_thread")]
async fn readiness_preserves_one_message_until_one_hardware_claim() -> TestResult {
    // Given: a masked Message-routed completion with pending hardware notification.
    let (device, worker) = device(ResourceConfig::default());
    let handle = device.handle();
    let [submission, _, _] = create_queue(&handle, 1).await?;
    let endpoint = handle.register_endpoint();
    write_u32(&endpoint, i(IrqSourceRegisters::ROUTE_KIND.offset())?, 1).await?;
    write_u64(&endpoint, i(IrqSourceRegisters::ROUTE_VECTOR.offset())?, 41).await?;
    write_u32(&endpoint, i(IrqSourceRegisters::ENABLED.offset())?, 1).await?;
    submit_copy(&handle, submission.base(), 0).await?;
    write_u64(&endpoint, q(QueueRegisters::DOORBELL.offset())?, 1).await?;
    let worker = tokio::spawn(worker.run());
    device.wait_for_progress_after(0).await;
    let first = tokio::spawn({
        let handle = handle.clone();
        async move {
            handle
                .wait_irq_notification_ready_at_epoch(DeviceEpoch::initial())
                .await
        }
    });
    let second = tokio::spawn({
        let handle = handle.clone();
        async move {
            handle
                .wait_irq_notification_ready_at_epoch(DeviceEpoch::initial())
                .await
        }
    });
    tokio::task::yield_now().await;
    assert!(!first.is_finished());
    assert!(!second.is_finished());
    // When: an unmask write wakes both non-consuming readiness waiters.
    write_u32(&endpoint, i(IrqSourceRegisters::MASKED.offset())?, 0).await?;
    assert!(first.await??);
    assert!(second.await??);
    // Then: both see readiness but only one hardware claim obtains the message.
    let Some(IrqNotification::Message(message)) = handle
        .take_irq_notification_at_epoch(DeviceEpoch::initial())
        .await?
    else {
        return Err("message required".into());
    };
    assert_eq!(message.vector().raw(), 41);
    assert_eq!(message.id().raw(), 0);
    assert_eq!(
        handle
            .take_irq_notification_at_epoch(DeviceEpoch::initial())
            .await?,
        None
    );
    device.request_shutdown().await;
    assert!(
        !handle
            .wait_irq_notification_ready_at_epoch(DeviceEpoch::initial())
            .await?
    );
    worker.await?;
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn readiness_wait_returns_shutdown_without_notification() -> TestResult {
    // Given: a readiness waiter canceled before a second waiter starts.
    let (device, _worker) = device(ResourceConfig::default());
    let handle = device.handle();
    let cancelled = tokio::spawn({
        let handle = handle.clone();
        async move {
            handle
                .wait_irq_notification_ready_at_epoch(DeviceEpoch::initial())
                .await
        }
    });
    tokio::task::yield_now().await;
    cancelled.abort();
    assert!(cancelled.await.is_err_and(|error| error.is_cancelled()));
    let wait = tokio::spawn({
        let handle = handle.clone();
        async move {
            handle
                .wait_irq_notification_ready_at_epoch(DeviceEpoch::initial())
                .await
        }
    });
    // When: shutdown occurs without any produced IRQ.
    device.request_shutdown().await;
    // Then: readiness terminates and no notification is manufactured.
    assert!(!wait.await??);
    assert_eq!(
        handle
            .take_irq_notification_at_epoch(DeviceEpoch::initial())
            .await?,
        None
    );
    Ok(())
}
