use pdma_model_contract::{CopyRequest, IrqNotification, IrqRoute, IrqVector, TraceEventKind};
use pdma_model_device::AsyncDevice;
use pdma_model_host::ModelError;
use pdma_model_host::ModelResult;
use pdma_model_host::{AsyncDriver, IrqDelivery, IrqHandler, IrqHandlerFuture, SoftwareCpu};
use tokio::sync::oneshot;

struct FailingHandler {
    started: Option<oneshot::Sender<()>>,
}

impl IrqHandler for FailingHandler {
    fn handle(&mut self, _notification: IrqNotification) -> IrqHandlerFuture<'_> {
        Box::pin(async move {
            if let Some(started) = self.started.take() {
                let _ = started.send(());
            }
            Err(ModelError::DriverQueueFull)
        })
    }
}

#[tokio::test(flavor = "current_thread")]
async fn handler_error_terminates_without_a_successful_delivery() -> ModelResult<()> {
    // Given: one pending IRQ and a handler that reports a typed failure.
    let (device, worker) = AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        pdma_model_contract::ResourceConfig::default(),
        std::sync::Arc::new(hardware_interface_runtime::NoopRegisterObserver),
    ));
    let worker = tokio::spawn(worker.run());
    let mut device_port = device.handle();
    let cpu = SoftwareCpu::new();
    let (mut driver, src, dst) = cpu
        .run_main(async {
            let driver = AsyncDriver::setup_on(&mut device_port, 4, 4).await?;
            let source = driver.completion_irq_source();
            device_port
                .configure_irq_route(source, IrqRoute::Message(IrqVector::new(9)))
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
            Ok::<_, ModelError>((driver, src, dst))
        })
        .await?;
    let (started_tx, started_rx) = oneshot::channel();
    let (delivery, delivery_handle) = IrqDelivery::new(device_port.clone(), cpu.clone())
        .register_handler(
            IrqVector::new(9),
            FailingHandler {
                started: Some(started_tx),
            },
        );
    let delivery = tokio::spawn(delivery.run());

    // When: the notification reaches the failing handler.
    cpu.run_main(driver.submit_copy_on(&mut device_port, CopyRequest::new(src, dst, 4)))
        .await?;
    let _ = started_rx.await;
    let result = delivery.await.expect("delivery task joins cleanly");

    // Then: the actor propagates the failure and traces no successful completion.
    assert_eq!(result, Err(ModelError::DriverQueueFull));
    let trace = delivery_handle.trace_events().await;
    let trace = trace
        .iter()
        .map(|event| (event.kind(), event.detail()))
        .collect::<Vec<_>>();
    assert_eq!(
        trace,
        [
            (TraceEventKind::IrqHandlerStarted, "message"),
            (TraceEventKind::IrqHandlerCompleted, "Error"),
        ]
    );
    device.request_shutdown().await;
    worker.await.expect("device worker joins cleanly");
    Ok(())
}
