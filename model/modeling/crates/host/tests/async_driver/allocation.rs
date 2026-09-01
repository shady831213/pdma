use super::support::BlockingPort;
use hardware_interface_runtime::SoftwareCpu;
use pdma_model_contract::semantic::{BufferId, CommandId, CopyRequest};
use pdma_model_device::AsyncDevice;
use pdma_model_host::{AsyncDriver, DriverError, ManualDriver, PortError, compat::AsyncDevicePort};
use std::{future::poll_fn, task::Poll};

#[tokio::test(flavor = "current_thread")]
async fn fallible_allocation_propagates_through_both_driver_facades() {
    // Given: CPU-bound and manual drivers on separate queues of one async device.
    let (device, _worker) = AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        pdma_model_contract::semantic::ResourceConfig::default(),
    ));
    let handle = device.handle();
    let mut port = AsyncDevicePort::new(handle.clone());
    let async_driver = SoftwareCpu::new()
        .run_main(AsyncDriver::setup_on(&mut port, 1, 1))
        .await
        .expect("async setup");
    let manual_driver = ManualDriver::setup_on(&mut port, 1, 1)
        .await
        .expect("manual setup");

    // When: both facades allocate through the same public async port.
    let async_object = async_driver
        .allocate_buffer_on(&mut port, 3)
        .await
        .expect("async allocation");
    let manual_object = manual_driver
        .allocate_buffer_on(&mut port, 5)
        .await
        .expect("manual allocation");

    // Then: each semantic ID correlates with one exact committed DMA mapping.
    let async_region = handle
        .region_for_object(async_object)
        .await
        .expect("async mapping");
    let manual_region = handle
        .region_for_object(manual_object)
        .await
        .expect("manual mapping");
    assert_eq!(async_object, BufferId::new(0));
    assert_eq!(async_region.object(), async_object);
    assert_eq!(async_region.base().raw(), 0x1_0000);
    assert_eq!(async_region.length(), 3);
    assert_eq!(manual_object, BufferId::new(1));
    assert_eq!(manual_region.object(), manual_object);
    assert_eq!(manual_region.base().raw(), 0x1_0040);
    assert_eq!(manual_region.length(), 5);
}

#[tokio::test(flavor = "current_thread")]
async fn cancelled_or_exhausted_allocation_commits_no_mapping() {
    // Given: an async driver whose allocation port can suspend before device mutation.
    let (device, _worker) = AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        pdma_model_contract::semantic::ResourceConfig::default(),
    ));
    let handle = device.handle();
    handle.enable_tracing().await;
    let mut port = BlockingPort::new(handle.clone());
    let cpu = SoftwareCpu::new();
    let mut driver = cpu
        .run_main(AsyncDriver::setup_on(&mut port, 1, 1))
        .await
        .expect("driver setup");
    let trace_before = handle.trace_events().await;
    port.block_allocation();
    let control = port.clone();
    let mut pending = Box::pin(driver.allocate_buffer_on(&mut port, 4));
    cpu.run_main(poll_fn(|context| {
        assert!(pending.as_mut().poll(context).is_pending());
        Poll::Ready(())
    }))
    .await;
    control.wait_allocation_entered().await;

    // When: the pending future is dropped and address-space exhaustion follows.
    drop(pending);
    control.allow_allocation();
    let exhausted = driver.allocate_buffer_on(&mut port, usize::MAX).await;

    // Then: neither rejection mutates mapping identity, bytes, trace, or driver accounting.
    assert_eq!(
        exhausted,
        Err(DriverError::Port(PortError::AllocationExhausted))
    );
    assert_eq!(handle.region_for_object(BufferId::new(0)).await, None);
    assert_eq!(handle.trace_events().await, trace_before);
    let object = driver
        .allocate_buffer_on(&mut port, 4)
        .await
        .expect("first committed allocation");
    let region = handle
        .region_for_object(object)
        .await
        .expect("committed mapping");
    assert_eq!(object, BufferId::new(0));
    assert_eq!(region.base().raw(), 0x1_0000);
    assert_eq!(handle.read_buffer(object, 0, 4).await, Ok(vec![0; 4]));
    let destination = driver
        .allocate_buffer_on(&mut port, 4)
        .await
        .expect("destination allocation");
    let submitted = cpu
        .run_main(driver.submit_copy_on(&mut port, CopyRequest::new(object, destination, 4)))
        .await;
    assert_eq!(submitted, Ok(CommandId::new(0)));
}
