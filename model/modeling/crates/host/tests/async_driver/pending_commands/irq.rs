mod recovery;
use recovery::Recovery;

use super::support::{GatedAccess, drop_pending};
use hardware_interface_runtime::{InterruptSource, SharedMemory, SoftwareCpu};
use pdma_model_contract::semantic::{
    CopyRequest, DeviceEpoch, IrqNotification, IrqRoute, IrqVector, ResourceConfig,
    WholeDeviceReset,
};
use pdma_model_device::AsyncDevice;
use pdma_model_host::{
    AsyncDriver, DriverPort,
    adapter::abi::{
        AsyncDeviceAccess, PdmaExactInterruptClaim, PdmaInterruptSource, PdmaRegisterPort,
        PdmaRegisterPortError,
    },
};
use std::sync::Arc;
use std::{future::poll_fn, task::Poll};

#[tokio::test(flavor = "current_thread")]
async fn cancelled_irq_result_recovers_the_consumed_message() {
    cancelled_claim(0x224, Recovery::Claim).await;
}

#[tokio::test(flavor = "current_thread")]
async fn cancelled_irq_response_recovers_before_source_command() {
    cancelled_claim(0x260, Recovery::SourceCommand).await;
}

#[tokio::test(flavor = "current_thread")]
async fn independent_dispatcher_recovers_the_port_claim() {
    cancelled_claim(0x260, Recovery::Dispatcher).await;
}

#[tokio::test(flavor = "current_thread")]
async fn failed_irq_result_read_blocks_source_overwrite_and_remains_recoverable() {
    cancelled_claim(0x224, Recovery::ReadFailure).await;
}

#[tokio::test(flavor = "current_thread")]
async fn failed_irq_response_read_retains_partial_response() {
    cancelled_claim(0x260, Recovery::ReadFailure).await;
}

#[tokio::test(flavor = "current_thread")]
async fn observer_failure_is_typed_and_retains_irq_consumption() {
    cancelled_claim(0x260, Recovery::ObserverFailure).await;
}

#[tokio::test(flavor = "current_thread")]
async fn reset_invalidates_cancelled_irq_response_with_a_typed_error() {
    cancelled_claim(0x260, Recovery::Reset).await;
}

#[tokio::test(flavor = "current_thread")]
async fn malformed_irq_result_is_not_reported_as_a_notification() {
    cancelled_claim(0x224, Recovery::MalformedResult).await;
}

#[tokio::test(flavor = "current_thread")]
async fn irq_claim_resumed_on_wrong_cpu_stops_before_another_access() {
    cancelled_claim(0x224, Recovery::WrongCpu).await;
}

#[tokio::test(flavor = "current_thread")]
async fn cancellation_before_irq_command_keeps_hardware_message_available() {
    cancelled_claim(0x224, Recovery::BeforeCommand).await;
}

#[tokio::test(flavor = "current_thread")]
async fn each_irq_response_boundary_preserves_consumed_notification() {
    for offset in [
        0x240, 0x248, 0x250, 0x254, 0x258, 0x25c, 0x260, 0x268, 0x270,
    ] {
        cancelled_claim(offset, Recovery::Claim).await;
    }
}

#[tokio::test(flavor = "current_thread")]
async fn dispatcher_cancellation_is_recovered_before_port_mask() {
    cancelled_claim(0x260, Recovery::SourceCancelled).await;
}

#[tokio::test(flavor = "current_thread")]
async fn global_discovery_response_does_not_invalidate_new_epoch_claim() {
    cancelled_claim(0x260, Recovery::AfterReset).await;
}

#[tokio::test(flavor = "current_thread")]
async fn empty_snapshot_preserves_host_owned_new_epoch_notification() {
    cancelled_claim(0x260, Recovery::SnapshotAfterReset).await;
}

#[tokio::test(flavor = "current_thread")]
async fn readiness_waiter_wakes_for_another_clones_cancelled_consumption() {
    cancelled_claim(0x260, Recovery::WaitingDispatcher).await;
}

#[tokio::test(flavor = "current_thread")]
async fn independently_constructed_adapter_recovers_with_explicit_shared_context() {
    cancelled_claim(0x260, Recovery::IndependentAdapter).await;
}

#[expect(clippy::expect_used, reason = "real-device regression assertions")]
async fn cancelled_claim(offset: u32, recovery: Recovery) {
    // Given: a real message source with one completed DMA command.
    let (device, worker) = AsyncDevice::new(
        pdma_model_device::PdmaMemoryProvisioning::shared(Arc::new(SharedMemory::new())),
        ResourceConfig::default(),
    );
    let worker = tokio::spawn(worker.run());
    let access = GatedAccess::new(AsyncDeviceAccess::new(
        device.handle(),
        pdma_model_host::adapter::abi::PdmaIrqCoordinator::default(),
    ));
    SoftwareCpu::new().run_main(async {
        let epoch = if matches!(recovery, Recovery::AfterReset | Recovery::SnapshotAfterReset) {
            device.handle().reset(WholeDeviceReset::new(DeviceEpoch::initial())).await.expect("reset")
        } else { DeviceEpoch::initial() };
        let mut port = PdmaRegisterPort::new_at(access.clone(), epoch).expect("port");
        let mut driver = AsyncDriver::setup_on(&mut port, 1, 1).await.expect("driver");
        let queue = driver.queue_config_handle();
        let source = queue.completion_irq_source();
        port.configure_irq_route(source, IrqRoute::Message(IrqVector::new(41))).await.expect("route");
        port.enable_irq_source(source).await.expect("enable");
        port.unmask_irq_source(source).await.expect("unmask");
        let src = driver.allocate_buffer_on(&mut port, 1).await.expect("src");
        let dst = driver.allocate_buffer_on(&mut port, 1).await.expect("dst");
        let raw = access.inner();
        let mut waiting = Box::pin(raw.wait_interrupt_ready());
        if matches!(recovery, Recovery::WaitingDispatcher) { drop_pending(waiting.as_mut()).await; }
        driver.submit_copy_on(&mut port, CopyRequest::new(src, dst, 1)).await.expect("copy");
        device.wait_for_progress_after(0).await;
        if matches!(recovery, Recovery::BeforeCommand) { access.arm_before(9); }
        else { access.arm_after_at(9, offset); }
        // When: software drops the claim after consuming the message in hardware.
        if matches!(recovery, Recovery::WrongCpu) {
            let mut pending = Box::pin(port.try_wait_irq_notification());
            poll_fn(|cx| {
                assert!(pending.as_mut().poll(cx).is_pending());
                Poll::Ready(())
            }).await;
            let result = SoftwareCpu::new().run_main(pending.as_mut()).await;
            assert!(matches!(result, Err(PdmaRegisterPortError::Cpu(_))));
        } else if matches!(recovery, Recovery::SourceCancelled) {
            let mut source = PdmaInterruptSource::new(access.clone());
            let ready = source.wait_ready().await.expect("ready");
            drop_pending(source.claim(ready)).await;
        } else {
            drop_pending(port.try_wait_irq_notification()).await;
        }
        assert!(access.is_blocked());
        assert_eq!(device.handle().irq_notification_ready().await, matches!(recovery, Recovery::BeforeCommand));
        access.release();
        if matches!(recovery, Recovery::WaitingDispatcher) {
            poll_fn(|cx| {
                assert!(matches!(waiting.as_mut().poll(cx), Poll::Ready(Ok(true))));
                Poll::Ready(())
            }).await;
        }
        let Some(notification) = recovery.recover(&mut port, &access, &device, source, offset).await else { return; };

        // Then: recovery returns the original notification even though hardware is empty.
        assert!(matches!(notification, IrqNotification::Message(message) if message.source() == source.source()));
        device.request_shutdown().await;
        assert!(port.try_wait_irq_notification().await.expect("closed").is_none());
    }).await;
    worker.await.expect("worker joined");
}
