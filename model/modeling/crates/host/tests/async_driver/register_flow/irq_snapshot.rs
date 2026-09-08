use hardware_interface_runtime::{RegisterAccess, SharedMemory, SoftwareCpu};
use pdma_model_contract::abi::registers::GlobalRegisters;
use pdma_model_contract::semantic::{DeviceEpoch, IrqLine, IrqRoute, IrqVector, ResourceConfig};
use pdma_model_device::{AsyncDevice, PdmaMemoryProvisioning};
use pdma_model_host::{
    AsyncDriver, CopyRequest, DriverPort, PortError,
    adapter::abi::{AsyncDeviceAccess, PdmaRegisterPort},
};
use std::sync::Arc;

#[tokio::test(flavor = "current_thread")]
async fn masked_pending_line_remains_visible_without_assertion() {
    // Given: a completion source is enabled and masked on a line route.
    let (device, worker) = AsyncDevice::new(
        PdmaMemoryProvisioning::shared(Arc::new(SharedMemory::new())),
        ResourceConfig::default(),
    );
    let worker = tokio::spawn(worker.run());
    SoftwareCpu::new()
        .run_main(async {
            let mut port =
                PdmaRegisterPort::new(AsyncDeviceAccess::new(device.handle())).expect("port");
            let mut driver = AsyncDriver::setup_on(&mut port, 1, 1)
                .await
                .expect("driver");
            let source = driver.completion_irq_source();
            port.configure_irq_route(source, IrqRoute::Line(IrqLine::new(IrqVector::new(12))))
                .await
                .expect("line route");
            port.enable_irq_source(source).await.expect("enable");
            port.mask_irq_source(source).await.expect("mask");
            let buffer = driver
                .allocate_buffer_on(&mut port, 1)
                .await
                .expect("buffer");
            driver
                .submit_copy_on(&mut port, CopyRequest::new(buffer, buffer, 1))
                .await
                .expect("submit");
            device.wait_for_progress_after(0).await;

            // When: software observes the pending line without claiming an interrupt.
            let snapshot = port
                .irq_snapshot()
                .await
                .expect("snapshot")
                .expect("pending line");

            // Then: masking suppresses assertion while preserving the visible cause.
            assert!(!snapshot.asserted());
            assert_eq!(snapshot.cause().source_capability(), source);
        })
        .await;
    device.request_shutdown().await;
    worker.await.expect("worker");
}

#[tokio::test(flavor = "current_thread")]
async fn empty_snapshot_distinguishes_no_irq_from_stale_epoch() {
    // Given: an empty device and a valid host snapshot capability.
    let (device, _worker) = AsyncDevice::new(
        PdmaMemoryProvisioning::shared(Arc::new(SharedMemory::new())),
        ResourceConfig::default(),
    );
    SoftwareCpu::new()
        .run_main(async {
            let access = AsyncDeviceAccess::new(device.handle());
            let port = PdmaRegisterPort::new(access.clone()).expect("port");
            assert_eq!(port.irq_snapshot().await, Ok(None));

            // When: reset invalidates the snapshot capability without producing any IRQ.
            access
                .write(
                    u32::try_from(GlobalRegisters::RESET.offset()).expect("reset offset"),
                    &1_u32.to_le_bytes(),
                )
                .await
                .expect("reset");

            // Then: snapshot reports the stale epoch instead of an empty interrupt state.
            assert_eq!(
                port.irq_snapshot().await,
                Err(PortError::StaleEpoch {
                    expected: DeviceEpoch::new(1),
                    actual: DeviceEpoch::initial(),
                })
            );
        })
        .await;
}
