/// Adverse endpoint surface.
mod adverse;
/// Clone ordering and reset surface.
mod order_reset;
/// Shared surface fixtures.
mod support;
/// Exact happy endpoint surface.
mod surface;

use hardware_interface_runtime::RegisterAccess;
use pdma_model_contract::abi::registers::{IrqSourceRegisters, MmioAccessError, irq_offset};
use pdma_model_contract::semantic::ResourceConfig;
use pdma_model_device::{AsyncDevice, MmioRegisterError};
use std::sync::Arc;

#[tokio::test(flavor = "current_thread")]
async fn register_endpoint_surface() -> support::TestResult {
    surface::run().await
}

#[tokio::test(flavor = "current_thread")]
async fn register_endpoint_adverse() -> support::TestResult {
    adverse::run().await
}

#[tokio::test(flavor = "current_thread")]
async fn malformed_ack_width_is_typed_and_non_mutating() -> support::TestResult {
    // Given: one pending message and an observer with a stable pre-access transcript.
    let observer = Arc::new(support::RecordingObserver::default());
    let memory = Arc::new(hardware_interface_runtime::SharedMemory::new());
    let (device, worker) = AsyncDevice::new_observed(
        pdma_model_device::PdmaMemoryProvisioning::shared(memory),
        ResourceConfig::default(),
        observer.clone(),
    );
    let worker = tokio::spawn(worker.run());
    let handle = device.handle();
    let endpoint = handle.register_endpoint();
    let queue = support::create_message(&handle).await?;
    support::submit_copy(&handle, queue).await?;
    device.wait_for_progress_after(0).await;
    let before_state = handle.snapshot().await;
    let before_observations = observer.snapshot();
    let offset = irq_offset(0, 0, IrqSourceRegisters::ACK.offset())?;

    // When: the 4-byte acknowledgement register receives a wrong 8-byte write.
    let error = endpoint
        .write(offset, &0_u64.to_le_bytes())
        .await
        .expect_err("malformed acknowledgement width must fail");

    // Then: typed width rejection leaves notification readiness and observations intact.
    assert!(matches!(
        error,
        MmioRegisterError::Access(MmioAccessError::WrongWidth {
            offset: actual_offset,
            expected: 4,
            actual: 8,
        }) if actual_offset == offset
    ));
    assert_eq!(handle.snapshot().await, before_state);
    assert_eq!(observer.snapshot(), before_observations);
    assert!(support::claim(&handle).await?);
    device.request_shutdown().await;
    worker.await?;
    Ok(())
}
