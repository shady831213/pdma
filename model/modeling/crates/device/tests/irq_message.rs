#![allow(missing_docs)]

use pdma_model_contract::{
    desc::{CopyCommand, CopyRequest, ExternalCommand},
    ids::CommandId,
    irq::{IrqMessageId, IrqNotification, IrqRoute, IrqSource, IrqVector},
    queue::{EventRouting, QueueConfig, SoftwareOwnerId},
};
use pdma_model_device::AsyncDevice;

#[tokio::test(flavor = "current_thread")]
async fn message_irq_delivers_each_completion_publication_without_coalescing() {
    // Given: two successful copies routed to one enabled message-signaled IRQ source.
    let (runner, worker) = AsyncDevice::queue_port();
    let worker = tokio::spawn(worker.run());
    let handle = runner.handle();
    let queue = handle
        .create_queue(QueueConfig::new(
            4,
            4,
            EventRouting::QueueLocal,
            SoftwareOwnerId::new(0),
        ))
        .await
        .expect("queue setup is valid");
    let source = IrqSource::Completion(queue.completion_ring_id());
    let vector = IrqVector::new(11);
    handle
        .configure_irq_route(source, IrqRoute::Message(vector))
        .await
        .expect("inactive IRQ source route is configurable");
    handle
        .enable_irq_source(source)
        .await
        .expect("completion IRQ source exists");
    handle
        .unmask_irq_source(source)
        .await
        .expect("completion IRQ source exists");
    let src = handle.allocate_buffer(4).await;
    let first_dst = handle.allocate_buffer(4).await;
    let second_dst = handle.allocate_buffer(4).await;
    handle
        .write_buffer(src, 0, &[2, 4, 6, 8])
        .await
        .expect("source write is in bounds");
    for (command_id, dst) in [first_dst, second_dst].into_iter().enumerate() {
        handle
            .submit_command(
                queue.queue_id(),
                ExternalCommand::Copy(CopyCommand::new(
                    CommandId::new(command_id),
                    queue.queue_id(),
                    queue.completion_ring_id(),
                    CopyRequest::new(src, dst, 4),
                )),
            )
            .await
            .expect("copy submission has ring space");
    }

    // When: both copies retire and software waits for two IRQ notifications.
    handle
        .ring_doorbell(queue.queue_id())
        .await
        .expect("doorbell rings");
    let first = handle
        .wait_irq_notification()
        .await
        .expect("first IRQ message is delivered");
    let second = handle
        .wait_irq_notification()
        .await
        .expect("second IRQ message is delivered");
    runner.request_shutdown().await;
    worker.await.expect("runner joins cleanly");

    // Then: both publications retain distinct message identities on the configured vector.
    assert!(matches!(
        first,
        IrqNotification::Message(message)
            if message.id() == IrqMessageId::new(0)
                && message.vector() == vector
                && message.source() == source
    ));
    assert!(matches!(
        second,
        IrqNotification::Message(message)
            if message.id() == IrqMessageId::new(1)
                && message.vector() == vector
                && message.source() == source
    ));
}
