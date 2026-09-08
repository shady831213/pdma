use super::support::Fixture;
use hardware_interface_runtime::RegisterAccess;
use pdma_model_contract::{
    abi::registers::{IrqSourceRegisters, irq_offset},
    semantic::{DeviceEpoch, IrqMessageId, IrqNotification, IrqVector, ResourceConfig},
};

#[tokio::test(flavor = "current_thread")]
async fn message_irq_delivers_each_completion_publication_without_coalescing() {
    // Given: two copies routed to one message-signaled source.
    let (fixture, worker) = Fixture::new(ResourceConfig::default()).await;
    fixture
        .endpoint
        .write(
            irq_offset(0, 0, IrqSourceRegisters::ROUTE_VECTOR.offset()).expect("vector"),
            &11_u64.to_le_bytes(),
        )
        .await
        .expect("vector");
    fixture
        .endpoint
        .write(
            irq_offset(0, 0, IrqSourceRegisters::ROUTE_KIND.offset()).expect("route"),
            &1_u32.to_le_bytes(),
        )
        .await
        .expect("route");
    fixture.enable_irq(0).await;
    for slot in 0..2 {
        fixture.copy(slot, fixture.payload(4)).await;
    }
    let epoch = DeviceEpoch::initial();
    {
        let wait = fixture.handle.wait_irq_notification_ready_at_epoch(epoch);
        let mut wait = std::pin::pin!(wait);
        let first_poll = std::future::poll_fn(|context| {
            std::task::Poll::Ready(std::future::Future::poll(wait.as_mut(), context))
        })
        .await;
        assert!(first_poll.is_pending());
    }
    let worker = tokio::spawn(worker.run());

    // When: software waits for readiness and consumes two actual hardware outputs.
    fixture.publish(2).await;
    let mut messages = Vec::new();
    for _ in 0..2 {
        assert!(
            fixture
                .handle
                .wait_irq_notification_ready_at_epoch(epoch)
                .await
                .expect("ready")
        );
        messages.push(
            fixture
                .handle
                .take_irq_notification_at_epoch(epoch)
                .await
                .expect("take")
                .expect("notification"),
        );
    }
    fixture.device.request_shutdown().await;
    worker.await.expect("join");

    // Then: each publication has its own identity on the configured vector.
    for (index, notification) in messages.into_iter().enumerate() {
        match notification {
            IrqNotification::Message(message) => {
                assert_eq!(message.id(), IrqMessageId::new(index));
                assert_eq!(message.vector(), IrqVector::new(11));
                assert_eq!(
                    message.source(),
                    pdma_model_contract::semantic::IrqSource::Completion(
                        pdma_model_contract::semantic::CompletionRingId::new(0)
                    )
                );
            }
            IrqNotification::Line(_) => panic!("expected a message"),
        }
    }
}
