use crate::irq_delivery::{IrqDelivery, IrqHandler, IrqHandlerFuture};
use crate::{AsyncDriver, DriverError, DriverPort as _, compat::AsyncDevicePort};
use hardware_interface_runtime::SoftwareCpu;
use pdma_model_contract::semantic::{CopyRequest, IrqNotification, IrqRoute, IrqVector};
use pdma_model_device::AsyncDevice;
use tokio::sync::mpsc;

struct CountingHandler {
    calls: mpsc::UnboundedSender<()>,
}

impl IrqHandler for CountingHandler {
    fn handle(&mut self, _notification: IrqNotification) -> IrqHandlerFuture<'_> {
        Box::pin(async move {
            let _ = self.calls.send(());
            Ok(())
        })
    }
}

#[tokio::test(flavor = "current_thread")]
async fn shutdown_wins_over_a_ready_notification() -> Result<(), Box<dyn std::error::Error>> {
    // Given: a pending message notification and a delivery actor not yet running.
    let (device, worker) = AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        pdma_model_contract::semantic::ResourceConfig::default(),
        std::sync::Arc::new(hardware_interface_runtime::NoopRegisterObserver),
    ));
    let worker = tokio::spawn(worker.run());
    let mut device_port = AsyncDevicePort::new(device.handle());
    let cpu = SoftwareCpu::new();
    let (mut driver, src, dst) = cpu
        .run_main(async {
            let driver = AsyncDriver::setup_on(&mut device_port, 4, 4).await?;
            let source = driver.completion_irq_source();
            device_port
                .configure_irq_route(source, IrqRoute::Message(IrqVector::new(8)))
                .await?;
            device_port.enable_irq_source(source).await?;
            device_port.unmask_irq_source(source).await?;
            let src = driver
                .allocate_buffer_on(&mut device_port, 4)
                .await
                .expect("allocation");
            let dst = driver
                .allocate_buffer_on(&mut device_port, 4)
                .await
                .expect("allocation");
            driver
                .write_buffer_on(&mut device_port, src, &[1, 2, 3, 4])
                .await?;
            Ok::<_, DriverError>((driver, src, dst))
        })
        .await?;
    let (calls_tx, mut calls_rx) = mpsc::unbounded_channel();
    let (delivery, delivery_handle) = IrqDelivery::new(device_port.clone(), cpu.clone())
        .register_handler(IrqVector::new(8), CountingHandler { calls: calls_tx });
    cpu.run_main(driver.submit_copy_on(&mut device_port, CopyRequest::new(src, dst, 4)))
        .await?;
    let progress = device.wait_for_progress_after(0).await;
    assert_eq!(progress.steps, 1);

    // When: shutdown and the pending notification are both ready before actor polling begins.
    delivery_handle.request_shutdown();
    let summary = delivery.run().await?;

    // Then: deterministic shutdown priority prevents an extra handler turn.
    assert_eq!(summary.deliveries(), 0);
    assert!(calls_rx.try_recv().is_err());
    device.request_shutdown().await;
    worker.await.expect("device worker joins cleanly");
    Ok(())
}
