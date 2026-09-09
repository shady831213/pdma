use super::*;
use crate::runtime::worker::run_worker;
use pdma_model_contract::abi::registers::irq_offset;

#[tokio::test(flavor = "current_thread")]
async fn mmio_unmask_wakes_idle_worker_and_releases_one_retained_message() {
    // Given: a masked message, an idle worker and a suspended readiness waiter.
    let (handle, _allocator) = pending_message();
    let endpoint = handle.register_endpoint();
    let mask = irq_offset(0, 0, IrqSourceRegisters::MASKED.offset()).expect("mask offset");
    endpoint
        .write(mask, &1_u32.to_le_bytes())
        .await
        .expect("mask");
    let mut worker = Box::pin(run_worker(&handle.shared));
    let mut context = Context::from_waker(Waker::noop());
    assert!(worker.as_mut().poll(&mut context).is_pending());
    let mut waiting = Box::pin(handle.wait_irq_notification_ready_at_epoch(DeviceEpoch::initial()));
    assert!(waiting.as_mut().poll(&mut context).is_pending());

    // When: MMIO changes only the source, then the real worker runs the woken step.
    endpoint
        .write(mask, &0_u32.to_le_bytes())
        .await
        .expect("unmask");
    assert!(!handle.irq_notification_ready().await);
    assert!(worker.as_mut().poll(&mut context).is_pending());

    // Then: the idle IRQ stage wakes readiness and exposes exactly the original message.
    assert_eq!(waiting.as_mut().poll(&mut context), Poll::Ready(Ok(true)));
    assert!(
        matches!(handle.take_irq_notification_at_epoch(DeviceEpoch::initial()).await,
        Ok(Some(IrqNotification::Message(message))) if message.id().raw() == 0)
    );
    assert!(worker.as_mut().poll(&mut context).is_pending());
    assert_eq!(
        handle
            .take_irq_notification_at_epoch(DeviceEpoch::initial())
            .await,
        Ok(None)
    );
    handle.request_shutdown().await;
    worker.await;
}
