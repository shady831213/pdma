#![allow(missing_docs)]

use pdma_model_host::{DriverError, PortError};

#[path = "async_driver/allocation.rs"]
mod allocation;
#[path = "async_driver/cancellation.rs"]
mod cancellation;
#[path = "async_driver/poll_preflight.rs"]
mod poll_preflight;
#[path = "async_driver/repoll_guard.rs"]
mod repoll_guard;
#[path = "async_driver/semantics.rs"]
mod semantics;
#[path = "async_driver/stale_capability.rs"]
mod stale_capability;
#[path = "async_driver/support.rs"]
pub mod support;

use hardware_interface_runtime::SoftwareCpu;
use pdma_model_contract::semantic::{
    DeviceEpoch,
    desc::CopyRequest,
    ids::{CommandId, CompletionRingId},
    irq::IrqSource,
    queue::{EventRouting, QueueConfig, SoftwareOwnerId},
};
use pdma_model_device::AsyncDevice;
use pdma_model_host::{AsyncDriver, DriverPort, compat::AsyncDevicePort};

#[tokio::test(flavor = "current_thread")]
async fn setup_on_rejects_inactive_cpu_before_creating_queue() {
    // Given: a fresh manual device and no active modeled software CPU.
    let (device, _worker) = AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        pdma_model_contract::semantic::ResourceConfig::default(),
    ));
    let mut port = support::BlockingPort::new(device.handle());

    // When: software-bearing setup is attempted outside run_main.
    let off_cpu = AsyncDriver::setup_on(&mut port, 1, 1).await;

    // Then: setup is rejected before consuming the device's first queue ID.
    assert!(matches!(
        off_cpu,
        Err(DriverError::InactiveCpu(
            hardware_interface_runtime::CpuContextError::Inactive
        ))
    ));
    assert_eq!(port.create_calls(), 0);
    let driver = SoftwareCpu::new()
        .run_main(AsyncDriver::setup_on(&mut port, 1, 1))
        .await
        .expect("active software setup is valid");
    assert_eq!(
        driver.queue_id(),
        pdma_model_contract::semantic::QueueId::new(0)
    );
    assert_eq!(port.create_calls(), 1);
}

#[tokio::test(flavor = "current_thread")]
async fn cloned_drivers_share_depth_one_command_credit() {
    // Given: two driver capabilities for one depth-one queue.
    let cpu = SoftwareCpu::new();
    let (device, _worker) = AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        pdma_model_contract::semantic::ResourceConfig::default(),
    ));
    let mut port = AsyncDevicePort::new(device.handle());

    // When: one capability consumes the only slot and the other submits.
    let (first, second) = cpu
        .run_main(async {
            let mut first_driver = AsyncDriver::setup_on(&mut port, 1, 1).await?;
            let mut second_driver = first_driver.clone();
            let src = first_driver
                .allocate_buffer_on(&mut port, 4)
                .await
                .expect("allocation");
            let dst = first_driver
                .allocate_buffer_on(&mut port, 4)
                .await
                .expect("allocation");
            let first = first_driver
                .submit_copy_on(&mut port, CopyRequest::new(src, dst, 4))
                .await;
            let second = second_driver
                .submit_copy_on(&mut port, CopyRequest::new(src, dst, 4))
                .await;
            Ok::<_, DriverError>((first, second))
        })
        .await
        .expect("setup remains valid");

    // Then: shared accounting rejects the second submit before the device ring does.
    assert_eq!(first, Ok(CommandId::new(0)));
    assert_eq!(second, Err(DriverError::QueueFull));
}

#[tokio::test(flavor = "current_thread")]
async fn submit_copy_on_rejects_second_command_when_local_slot_is_full() {
    // Given: a queue-port AsyncDevice, handle, AsyncDriver::setup_on(&mut handle, 1, 4), src/dst buffers.
    let (runner, worker) = AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        pdma_model_contract::semantic::ResourceConfig::default(),
    ));
    let worker = tokio::spawn(worker.run());
    let mut handle = AsyncDevicePort::new(runner.handle());
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
            Ok::<_, DriverError>((first, second))
        })
        .await
        .expect("queue setup and first submit are valid");
    runner.request_shutdown().await;
    let _summary = worker.await.expect("runner joins cleanly");

    // Then: second submit returns DriverError::QueueFull.
    assert_eq!(first, CommandId::new(0));
    assert_eq!(second, Err(DriverError::QueueFull));
}

#[tokio::test(flavor = "current_thread")]
async fn irq_controls_are_available_through_async_device_port() {
    // Given: a manual device used through the host port trait.
    let (device, _worker) = AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        pdma_model_contract::semantic::ResourceConfig::default(),
    ));
    let queue = device
        .handle()
        .create_queue(QueueConfig::new(
            4,
            4,
            EventRouting::QueueLocal,
            SoftwareOwnerId::new(0),
        ))
        .await
        .expect("queue setup is valid");
    let valid_source = queue.completion_irq_source();
    let unknown_source = IrqSource::Completion(CompletionRingId::new(99));
    let unknown_capability =
        pdma_model_contract::semantic::EpochIrqSource::new(DeviceEpoch::initial(), unknown_source);

    // When: host code controls IRQ state through the AsyncDevicePort abstraction.
    let mut port = AsyncDevicePort::new(device.handle());
    let enable_result = DriverPort::enable_irq_source(&mut port, valid_source).await;
    let unmask_result = DriverPort::unmask_irq_source(&mut port, valid_source).await;
    let invalid_result = DriverPort::enable_irq_source(&mut port, unknown_capability).await;

    // Then: valid controls succeed and unknown controls fail without creating state.
    assert_eq!(enable_result, Ok(()));
    assert_eq!(unmask_result, Ok(()));
    assert_eq!(
        invalid_result,
        Err(PortError::UnknownIrqSource(unknown_source))
    );
}
