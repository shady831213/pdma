//! Task 18 exact register endpoint surface and adverse validation.

#[path = "register_endpoint_surface/adverse.rs"]
/// Adverse endpoint surface.
pub mod adverse;
#[path = "register_endpoint_surface/order_reset.rs"]
/// Clone ordering and reset surface.
pub mod order_reset;
#[path = "register_endpoint_surface/support.rs"]
/// Shared surface fixtures.
pub mod support;
#[path = "register_endpoint_surface/surface.rs"]
/// Exact happy endpoint surface.
pub mod surface;

use hardware_interface_runtime::RegisterAccess;
use pdma_model_contract::abi::mmio::{
    MmioAccessError, PdmaMmioMap, QueueResponseRegistersRaw, RegisterId,
};
use pdma_model_contract::semantic::ResourceConfig;
use pdma_model_device::{AsyncDevice, DeviceCore, MmioRegisterError};
use std::sync::Arc;

#[tokio::test(flavor = "current_thread")]
async fn task18_register_endpoint_surface() -> support::TestResult {
    surface::run().await
}

#[tokio::test(flavor = "current_thread")]
async fn task18_register_endpoint_adverse() -> support::TestResult {
    adverse::run().await
}

#[tokio::test(flavor = "current_thread")]
async fn malformed_claim_width_is_typed_and_non_mutating() -> support::TestResult {
    // Given: one pending message and an observer with a stable pre-claim transcript.
    let observer = Arc::new(support::RecordingObserver::default());
    let memory = Arc::new(hardware_interface_runtime::SharedMemory::new());
    let (device, worker) = AsyncDevice::from_core_observed(
        DeviceCore::new(memory, ResourceConfig::default()),
        observer.clone(),
    );
    let worker = tokio::spawn(worker.run());
    let handle = device.handle();
    let endpoint = handle.register_endpoint();
    let queue = support::create_message(&handle).await?;
    support::submit_copy(&handle, queue).await?;
    handle.ring_doorbell(queue).await?;
    device.wait_for_progress_after(0).await;
    let before_state = handle.snapshot().await;
    let before_observations = observer.snapshot();
    let register = PdmaMmioMap::register(RegisterId::ICommand).ok_or("missing claim register")?;
    let offset = u32::try_from(register.offset())?;

    // When: the 4-byte claim command is presented with a supported but wrong 8-byte width.
    let error = endpoint
        .write(offset, &0_u64.to_le_bytes())
        .await
        .expect_err("malformed claim width must fail");

    // Then: the typed width error precedes notification consumption and observation emission.
    assert_eq!(QueueResponseRegistersRaw::ZERO.as_bytes().len(), 72);
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
    assert!(endpoint.claim_notification_exact().await?.is_some());
    device.request_shutdown().await;
    worker.await?;
    Ok(())
}
