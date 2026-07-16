#![allow(missing_docs)]

#[path = "async_driver/cancellation.rs"]
mod cancellation;
#[path = "async_driver/poll_preflight.rs"]
mod poll_preflight;
#[path = "async_driver/repoll_guard.rs"]
mod repoll_guard;
#[path = "async_driver/semantics.rs"]
mod semantics;
#[path = "async_driver/support.rs"]
pub mod support;

use pdma_model_contract::{
    ModelError,
    desc::CopyRequest,
    ids::{CommandId, CompletionRingId},
    irq::IrqSource,
    queue::{EventRouting, QueueConfig, SoftwareOwnerId},
};
use pdma_model_device::{AsyncDevice, SyncDevice};
use pdma_model_host::{AsyncDevicePort, AsyncDriver, SoftwareCpu};

#[tokio::test(flavor = "current_thread")]
async fn setup_on_rejects_inactive_cpu_before_creating_queue() {
    // Given: a fresh manual device and no active modeled software CPU.
    let (device, _worker) = AsyncDevice::queue_port();
    let mut port = support::BlockingPort::new(device.handle());

    // When: software-bearing setup is attempted outside run_main.
    let off_cpu = AsyncDriver::setup_on(&mut port, 1, 1).await;

    // Then: setup is rejected before consuming the device's first queue ID.
    assert!(matches!(off_cpu, Err(ModelError::SoftwareCpuInactive)));
    assert_eq!(port.create_calls(), 0);
    let driver = SoftwareCpu::new()
        .run_main(AsyncDriver::setup_on(&mut port, 1, 1))
        .await
        .expect("active software setup is valid");
    assert_eq!(driver.queue_id(), pdma_model_contract::QueueId::new(0));
    assert_eq!(port.create_calls(), 1);
}

#[tokio::test(flavor = "current_thread")]
async fn cloned_drivers_share_depth_one_command_credit() {
    // Given: two driver capabilities for one depth-one queue.
    let cpu = SoftwareCpu::new();
    let mut device = SyncDevice::new();

    // When: one capability consumes the only slot and the other submits.
    let (first, second) = cpu
        .run_main(async {
            let mut first_driver = AsyncDriver::setup_on(&mut device, 1, 1).await?;
            let mut second_driver = first_driver.clone();
            let src = first_driver.allocate_buffer_on(&mut device, 4).await;
            let dst = first_driver.allocate_buffer_on(&mut device, 4).await;
            let first = first_driver
                .submit_copy_on(&mut device, CopyRequest::new(src, dst, 4))
                .await;
            let second = second_driver
                .submit_copy_on(&mut device, CopyRequest::new(src, dst, 4))
                .await;
            Ok::<_, ModelError>((first, second))
        })
        .await
        .expect("setup remains valid");

    // Then: shared accounting rejects the second submit before the device ring does.
    assert_eq!(first, Ok(CommandId::new(0)));
    assert_eq!(second, Err(ModelError::DriverQueueFull));
}

#[tokio::test(flavor = "current_thread")]
async fn submit_copy_on_rejects_second_command_when_local_slot_is_full() {
    // Given: a queue-port AsyncDevice, handle, AsyncDriver::setup_on(&mut handle, 1, 4), src/dst buffers.
    let (runner, worker) = AsyncDevice::queue_port();
    let worker = tokio::spawn(worker.run());
    let mut handle = runner.handle();
    let (first, second) = SoftwareCpu::new()
        .run_main(async {
            let mut driver = AsyncDriver::setup_on(&mut handle, 1, 4).await?;
            let src = driver.allocate_buffer_on(&mut handle, 4).await;
            let dst = driver.allocate_buffer_on(&mut handle, 4).await;

            // When: first async copy submit succeeds, second submit before polling is attempted.
            let first = driver
                .submit_copy_on(&mut handle, CopyRequest::new(src, dst, 4))
                .await?;
            let second = driver
                .submit_copy_on(&mut handle, CopyRequest::new(src, dst, 4))
                .await;
            Ok::<_, ModelError>((first, second))
        })
        .await
        .expect("queue setup and first submit are valid");
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
