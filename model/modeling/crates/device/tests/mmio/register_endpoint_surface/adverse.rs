use super::support::{RecordingObserver, TestResult, contend_core, create_message, submit_copy};
use hardware_interface_runtime::{RegisterAccess, RegisterBusBase};
use pdma_model_contract::abi::registers::{QueueRegisters, queue_offset};
use pdma_model_contract::semantic::{DeviceEpoch, ResourceConfig};
use pdma_model_device::AsyncDevice;
use std::future::Future;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};

pub(super) async fn run() -> TestResult {
    // Given: one observed endpoint and a real pending message.
    let observer = Arc::new(RecordingObserver::default());
    let memory = Arc::new(hardware_interface_runtime::SharedMemory::new());
    let allocator = hardware_interface_runtime::SharedMemoryAllocator::new(
        memory.clone(),
        hardware_interface_runtime::MemoryAddress::new(0x1000),
    );
    let (device, worker) =
        AsyncDevice::new_observed(memory.clone(), ResourceConfig::default(), observer.clone());
    let worker = tokio::spawn(worker.run());
    let handle = device.handle();
    let endpoint = handle
        .register_endpoint()
        .at_mmio_base(RegisterBusBase::new(0x1_0000_0000));
    let submission = create_message(&handle, &allocator).await?;
    submit_copy(&handle, &allocator, submission).await?;
    device.wait_for_progress_after(0).await;
    let before_state = handle.snapshot().await;
    let before_failures = observer.snapshot();

    // When: malformed accesses and operations cancelled while waiting for the core occur.
    assert!(endpoint.read(0, &mut [0_u8; 1]).await.is_err());
    assert!(endpoint.read(6, &mut [0_u8; 4]).await.is_err());
    assert!(endpoint.read(12, &mut [0_u8; 4]).await.is_err());
    assert!(endpoint.write(0, &[0; 4]).await.is_err());
    assert!(
        endpoint
            .write(
                queue_offset(0, QueueRegisters::ENABLE.offset())?,
                &u32::MAX.to_le_bytes()
            )
            .await
            .is_err()
    );
    assert_eq!(observer.snapshot(), before_failures);
    assert_eq!(handle.snapshot().await, before_state);
    let held = contend_core(&handle, memory).await?;
    let before_cancel = observer.snapshot();
    let mut response = [0; 4];
    {
        let mut read = Box::pin(endpoint.read(0, &mut response));
        let mut context = Context::from_waker(Waker::noop());
        assert!(matches!(read.as_mut().poll(&mut context), Poll::Pending));
    }
    {
        let mut claim = Box::pin(handle.take_irq_notification_at_epoch(DeviceEpoch::initial()));
        let mut context = Context::from_waker(Waker::noop());
        assert!(matches!(claim.as_mut().poll(&mut context), Poll::Pending));
    }
    assert_eq!(observer.snapshot(), before_cancel);
    held.release()?;

    // Then: cancellation consumes no message; the available output is returned exactly once.
    assert!(super::support::claim(&handle).await?);
    assert!(!super::support::claim(&handle).await?);
    assert_eq!(observer.snapshot(), before_cancel);
    println!("malformed_observations=0;cancelled_observations=0;message_consumed_once=true");
    device.request_shutdown().await;
    worker.await?;
    Ok(())
}
