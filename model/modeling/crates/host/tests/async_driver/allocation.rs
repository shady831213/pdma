use super::support::BlockingPort;
use hardware_interface_runtime::SoftwareCpu;
use pdma_model_contract::semantic::CommandId;
use pdma_model_device::AsyncDevice;
use pdma_model_host::CopyRequest;
use pdma_model_host::{AsyncDriver, DriverError, ManualDriver, PortError};
use std::{future::poll_fn, task::Poll};

#[tokio::test(flavor = "current_thread")]
async fn fallible_allocation_propagates_through_both_driver_facades() {
    // Given: CPU-bound and manual drivers on separate queues of one async device.
    let storage = std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new());
    let memory = pdma_model_host::adapter::abi::PdmaHostMemory::new(storage.clone());
    let (device, _worker) = AsyncDevice::new(
        memory.memory_access(),
        pdma_model_contract::semantic::ResourceConfig::default(),
    );
    let handle = device.handle();
    let mut port = super::support::BlockingPort::new(handle.clone(), memory.clone());
    let cpu = SoftwareCpu::new();
    let async_driver = cpu
        .run_main(AsyncDriver::setup_on(&mut port, 1, 1))
        .await
        .expect("async setup");
    let manual_driver = cpu
        .run_main(ManualDriver::setup_on(&mut port, 1, 1))
        .await
        .expect("manual setup");

    // When: both facades allocate through the same public async port.
    let async_object = cpu
        .run_main(async_driver.allocate_buffer_on(&mut port, 3))
        .await
        .expect("async allocation");
    let manual_object = cpu
        .run_main(manual_driver.allocate_buffer_on(&mut port, 5))
        .await
        .expect("manual allocation");

    // Then: each semantic ID correlates with one exact committed DMA mapping.
    assert_ne!(async_object, manual_object);
    assert_eq!(
        cpu.run_main(async_driver.read_buffer_on(&port, async_object, 3))
            .await,
        Ok(vec![0; 3])
    );
    assert_eq!(
        cpu.run_main(manual_driver.read_buffer_on(&port, manual_object, 5))
            .await,
        Ok(vec![0; 5])
    );
    assert_eq!(
        storage
            .regions()
            .iter()
            .rev()
            .take(2)
            .map(|region| region.length())
            .collect::<Vec<_>>(),
        [5, 3]
    );
}

#[tokio::test(flavor = "current_thread")]
async fn cancelled_or_exhausted_allocation_commits_no_mapping() {
    // Given: an async driver whose allocation port can suspend before device mutation.
    let storage = std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new());
    let memory = pdma_model_host::adapter::abi::PdmaHostMemory::new(storage.clone());
    let (device, _worker) = AsyncDevice::new(
        memory.memory_access(),
        pdma_model_contract::semantic::ResourceConfig::default(),
    );
    let handle = device.handle();
    handle.enable_tracing().await;
    let mut port = BlockingPort::new(handle.clone(), memory.clone());
    let cpu = SoftwareCpu::new();
    let mut driver = cpu
        .run_main(AsyncDriver::setup_on(&mut port, 1, 1))
        .await
        .expect("driver setup");
    let trace_before = handle.trace_events().await;
    let generation_before = storage.revision();
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
    let exhausted = cpu
        .run_main(driver.allocate_buffer_on(&mut port, usize::MAX))
        .await;

    // Then: neither rejection mutates mapping identity, bytes, trace, or driver accounting.
    assert_eq!(
        exhausted,
        Err(DriverError::Port(PortError::AllocationExhausted))
    );
    assert_eq!(storage.revision(), generation_before);
    assert_eq!(handle.trace_events().await, trace_before);
    let object = cpu
        .run_main(driver.allocate_buffer_on(&mut port, 4))
        .await
        .expect("first committed allocation");
    assert_eq!(
        cpu.run_main(driver.read_buffer_on(&port, object, 4)).await,
        Ok(vec![0; 4])
    );
    let destination = cpu
        .run_main(driver.allocate_buffer_on(&mut port, 4))
        .await
        .expect("destination allocation");
    let submitted = cpu
        .run_main(driver.submit_copy_on(&mut port, CopyRequest::new(object, destination, 4)))
        .await;
    assert_eq!(submitted, Ok(CommandId::new(0)));
}
