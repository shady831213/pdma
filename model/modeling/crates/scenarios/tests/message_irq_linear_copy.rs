#![allow(missing_docs)]

use pdma_model_host::ModelError;
use pdma_model_host::ModelResult;

#[path = "message_irq_linear_copy/support.rs"]
pub mod support;

use pdma_model_contract::semantic::EpochQueue;
use pdma_model_contract::{CopyRequest, IrqRoute, IrqVector};
use pdma_model_device::AsyncDevice;
use pdma_model_host::{
    AsyncDevicePort, AsyncDriver, AsyncPdmaBus, BusDevicePort, IrqDelivery, MmioDeviceBus,
    SoftwareCpu, stateful_irq_handler,
};
use support::{
    LinearCopyHandler, MainOutcome, RecordingMmioTransport, ScenarioResult, SoftwarePhase,
    assert_outcome, start_trace,
};
use tokio::sync::oneshot;

#[tokio::test(flavor = "current_thread")]
async fn message_irq_linear_copy_runs_main_handler_main_end_to_end() -> ModelResult<()> {
    // Given: independent device, software-main, host-delivery, and handler actors.
    let (device, worker) = AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        pdma_model_contract::ResourceConfig::default(),
        std::sync::Arc::new(hardware_interface_runtime::NoopRegisterObserver),
    ));
    let worker = tokio::spawn(worker.run());
    let handle = device.handle();
    start_trace(&handle).await;
    let transport = RecordingMmioTransport::new(handle.clone());
    let raw_port = MmioDeviceBus::new(transport.clone());
    let harness_port = raw_port.clone();
    let cpu = SoftwareCpu::new();
    let phase = cpu.local(SoftwarePhase::MainBeforeSubmit);
    let vector = IrqVector::new(64);
    // Test-only rendezvous: main consumes the handler's observation to prove main-handler-main.
    let (report_tx, report_rx) = oneshot::channel();
    let (port_tx, port_rx) = oneshot::channel();
    let (bootstrap_tx, bootstrap_rx) = oneshot::channel();
    let delivery_cpu = cpu.clone();
    let delivery_task = tokio::spawn(async move {
        let delivery_port = port_rx
            .await
            .map_err(|_| ModelError::IrqDeliveryUnavailable)?;
        let (bootstrap, runner) = IrqDelivery::new(delivery_port, delivery_cpu).defer();
        bootstrap_tx
            .send(bootstrap)
            .map_err(|_| ModelError::IrqDeliveryUnavailable)?;
        runner.run().await
    });

    // When: one complete software program configures MSI-style delivery and submits a copy.
    // The program never raises an IRQ itself: completion publication in the device worker
    // latches the source, and host delivery schedules the registered handler.
    let main = tokio::spawn(async move {
        cpu.run_main(async move {
            let mut main_port = BusDevicePort::new(raw_port)?;
            port_tx
                .send(main_port.clone())
                .map_err(|_| ModelError::IrqDeliveryUnavailable)?;
            let delivery = bootstrap_rx
                .await
                .map_err(|_| ModelError::IrqDeliveryUnavailable)?;
            let mut driver = AsyncDriver::setup_on(&mut main_port, 1, 4).await?;
            let queue = driver.queue_config_handle();
            let source_capability = driver.completion_irq_source();
            let source = source_capability.source();
            let handler = stateful_irq_handler(LinearCopyHandler {
                driver: driver.clone(),
                port: main_port.clone(),
                phase: phase.clone(),
                report: Some(report_tx),
            });
            delivery.register_handler(vector, handler)?;
            main_port
                .configure_irq_route(source_capability, IrqRoute::Message(vector))
                .await?;
            main_port.enable_irq_source(source_capability).await?;
            main_port.unmask_irq_source(source_capability).await?;

            let src = driver.allocate_buffer_on(&mut main_port, 4).await?;
            let dst = driver.allocate_buffer_on(&mut main_port, 4).await?;
            driver
                .write_buffer_on(&mut main_port, src, &[1, 2, 3, 4])
                .await?;
            let command = driver
                .submit_copy_on(&mut main_port, CopyRequest::new(src, dst, 4))
                .await?;
            let report = report_rx
                .await
                .map_err(|_| ModelError::IrqDeliveryUnavailable)?;
            let phase = phase.with(|phase| {
                assert_eq!(*phase, SoftwarePhase::Handler);
                *phase = SoftwarePhase::MainAfterHandler;
                *phase
            })?;
            let copied = driver.read_buffer_on(&main_port, dst, 4).await?;
            Ok::<_, ModelError>(MainOutcome {
                command,
                queue,
                source,
                report,
                copied,
                phase,
            })
        })
        .await
    });

    let outcome = main.await.expect("software main joins cleanly")?;
    device.request_shutdown().await;
    let delivery_summary = delivery_task.await.expect("host delivery joins cleanly")?;
    let device_summary = worker.await.expect("device worker joins cleanly");
    let queue = EpochQueue::new(outcome.queue.epoch(), outcome.queue.queue_id());
    let queue_status = AsyncPdmaBus::read_queue_status(&harness_port, queue)
        .await
        .expect("queue status remains observable");

    // Then: device-triggered message delivery runs handler atomically and returns to main.
    assert_outcome(&ScenarioResult {
        vector,
        outcome,
        queue_status,
        deliveries: delivery_summary.deliveries(),
        device_steps: device_summary.steps,
        device_trace: handle.trace_events().await,
        mmio_accesses: transport.accesses(),
    });
    Ok(())
}
