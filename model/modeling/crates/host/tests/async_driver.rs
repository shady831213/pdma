#![allow(missing_docs)]

use pdma_model_host::ModelError;

include!("async_driver/allocation.rs");
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
    DeviceEpoch, ExternalCommand, QueueConfigHandle,
    desc::{CopyCommand, CopyRequest},
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
            let src = first_driver
                .allocate_buffer_on(&mut device, 4)
                .await
                .expect("allocation");
            let dst = first_driver
                .allocate_buffer_on(&mut device, 4)
                .await
                .expect("allocation");
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
            let src = driver
                .allocate_buffer_on(&mut handle, 4)
                .await
                .expect("allocation");
            let dst = driver
                .allocate_buffer_on(&mut handle, 4)
                .await
                .expect("allocation");

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
    let valid_source = queue.completion_irq_source();
    let unknown_source = IrqSource::Completion(CompletionRingId::new(99));
    let unknown_capability =
        pdma_model_contract::EpochIrqSource::new(DeviceEpoch::initial(), unknown_source);

    // When: host code controls IRQ state through the AsyncDevicePort abstraction.
    let enable_result = AsyncDevicePort::enable_irq_source(&mut device, valid_source).await;
    let unmask_result = AsyncDevicePort::unmask_irq_source(&mut device, valid_source).await;
    let invalid_result = AsyncDevicePort::enable_irq_source(&mut device, unknown_capability).await;
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

#[tokio::test(flavor = "current_thread")]
async fn stale_queue_and_result_capabilities_commit_nothing() {
    // Given: one real queue and a same-ID capability from a later device epoch.
    let mut device = SyncDevice::new();
    let queue = device
        .create_queue(QueueConfig::new(
            2,
            2,
            EventRouting::QueueLocal,
            SoftwareOwnerId::new(0),
        ))
        .expect("queue setup is valid");
    let stale_epoch = queue.epoch().next().expect("initial epoch can advance");
    let stale_queue = QueueConfigHandle::new(stale_epoch, queue.directory_entry());
    let source = device.allocate_buffer(1).expect("allocation");
    let destination = device.allocate_buffer(1).expect("allocation");
    let command = ExternalCommand::Copy(CopyCommand::new(
        CommandId::new(0),
        queue.queue_id(),
        queue.completion_ring_id(),
        CopyRequest::new(source, destination, 1),
    ));
    device.enable_tracing();
    let before_submit = device.trace_events();

    // When: stale queue capabilities reach submission and doorbell commit boundaries.
    let stale_submit =
        AsyncDevicePort::submit_command(&mut device, stale_queue, command.clone()).await;
    let stale_doorbell = AsyncDevicePort::ring_doorbell(&mut device, stale_queue).await;

    // Then: both fail with the typed epoch error and leave trace and queue cursors unchanged.
    assert_ne!(queue, stale_queue);
    assert_eq!(
        stale_submit,
        Err(ModelError::StaleDeviceEpoch {
            expected: DeviceEpoch::initial(),
            actual: stale_epoch,
        })
    );
    assert_eq!(
        stale_doorbell,
        Err(ModelError::StaleDeviceEpoch {
            expected: DeviceEpoch::initial(),
            actual: stale_epoch,
        })
    );
    assert_eq!(device.trace_events(), before_submit);
    AsyncDevicePort::submit_command(&mut device, queue, command)
        .await
        .expect("current queue capability submits");
    AsyncDevicePort::ring_doorbell(&mut device, queue)
        .await
        .expect("current queue capability rings");
    device
        .run_until_idle(1)
        .expect("current command retires exactly once");
    let before_result_poll = device.trace_events();
    let stale_completion = AsyncDevicePort::poll_completion(&mut device, stale_queue).await;
    let stale_event = AsyncDevicePort::poll_queue_event(&mut device, stale_queue).await;
    assert_eq!(
        stale_completion,
        Err(ModelError::StaleDeviceEpoch {
            expected: DeviceEpoch::initial(),
            actual: stale_epoch,
        })
    );
    assert_eq!(
        stale_event,
        Err(ModelError::StaleDeviceEpoch {
            expected: DeviceEpoch::initial(),
            actual: stale_epoch,
        })
    );
    assert_eq!(device.trace_events(), before_result_poll);
    let current_completion = AsyncDevicePort::poll_completion(&mut device, queue)
        .await
        .expect("current result capability is valid");
    assert!(current_completion.is_some());
    println!(
        "queue epochs current={} stale={} stale_submit={stale_submit:?} \
         stale_doorbell={stale_doorbell:?} stale_completion={stale_completion:?} \
         stale_event={stale_event:?} trace_unchanged=true record_retained=true",
        queue.epoch().raw(),
        stale_epoch.raw(),
    );
}
