#![allow(missing_docs)]

use pdma_model_contract::{
    ModelError,
    desc::CopyRequest,
    ids::{CommandId, CompletionRingId},
    irq::IrqSource,
    queue::{EventRouting, QueueConfig, SoftwareOwnerId},
};
use pdma_model_device::{AsyncDevice, SyncDevice};
use pdma_model_host::{AsyncDevicePort, AsyncDriver};

#[tokio::test(flavor = "current_thread")]
async fn submit_copy_on_rejects_second_command_when_local_slot_is_full() {
    // Given: a queue-port AsyncDevice, handle, AsyncDriver::setup_on(&mut handle, 1, 4), src/dst buffers.
    let (runner, worker) = AsyncDevice::queue_port();
    let worker = tokio::spawn(worker.run());
    let mut handle = runner.handle();
    let mut driver = AsyncDriver::setup_on(&mut handle, 1, 4)
        .await
        .expect("queue setup is valid");
    let src = driver.allocate_buffer_on(&mut handle, 4).await;
    let dst = driver.allocate_buffer_on(&mut handle, 4).await;

    // When: first async copy submit succeeds, second submit before polling is attempted.
    let first = driver
        .submit_copy_on(&mut handle, CopyRequest::new(src, dst, 4))
        .await
        .expect("first submit has a free slot");
    let second = driver
        .submit_copy_on(&mut handle, CopyRequest::new(src, dst, 4))
        .await;
    runner.request_shutdown().await;
    let _summary = worker.await.expect("runner joins cleanly");

    // Then: second submit returns ModelError::DriverQueueFull.
    assert_eq!(first, CommandId::new(0));
    assert_eq!(second, Err(ModelError::DriverQueueFull));
}

#[tokio::test(flavor = "current_thread")]
async fn irq_controls_are_available_through_async_device_port() {
    // Given: a manual device used through the host port trait.
    let mut device = SyncDevice::new();
    let queue = device
        .create_queue(QueueConfig::new(
            4,
            4,
            EventRouting::QueueLocal,
            SoftwareOwnerId::new(0),
        ))
        .expect("queue setup is valid");
    let valid_source = IrqSource::Completion(queue.completion_ring_id());
    let unknown_source = IrqSource::Completion(CompletionRingId::new(99));

    // When: host code controls IRQ state through the AsyncDevicePort abstraction.
    let enable_result = AsyncDevicePort::enable_irq_source(&mut device, valid_source).await;
    let unmask_result = AsyncDevicePort::unmask_irq_source(&mut device, valid_source).await;
    let invalid_result = AsyncDevicePort::enable_irq_source(&mut device, unknown_source).await;
    let wait_result = AsyncDevicePort::wait_irq(&device).await;

    // Then: valid controls succeed, unknown controls fail, and wait-line access is callable.
    assert_eq!(enable_result, Ok(()));
    assert_eq!(unmask_result, Ok(()));
    assert_eq!(
        invalid_result,
        Err(ModelError::UnknownIrqSource(unknown_source))
    );
    assert_eq!(wait_result, None);
}
