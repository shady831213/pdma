#![allow(missing_docs)]

pub mod support;

use hardware_interface_runtime::SoftwareCpu;
use pdma_model_contract::semantic::EpochQueue;
use pdma_model_contract::semantic::{CopyRequest, IrqRoute, IrqVector};
use pdma_model_host::{
    AsyncDriver, DriverPort, adapter::abi::AsyncDeviceAccess, adapter::abi::PdmaRegisterPort,
    runtime::interrupt::InterruptError,
};
use std::sync::Arc;
use support::{
    DeliveryConfig, MainOutcome, RecordingMemoryAccess, RecordingRegisterObserver, ScenarioResult,
    SoftwarePhase, assert_outcome, fresh_device, spawn_delivery, start_trace,
};
use tokio::sync::oneshot;

#[tokio::test(flavor = "current_thread")]
async fn message_irq_linear_copy_runs_main_handler_main_end_to_end()
-> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let observer = Arc::new(RecordingRegisterObserver::default());
    let (device, worker) = fresh_device(observer.clone());
    let worker = tokio::spawn(worker.run());
    let handle = device.handle();
    start_trace(&handle).await;
    let access = RecordingMemoryAccess::new(AsyncDeviceAccess::new(handle.clone()));
    let cpu = Arc::new(SoftwareCpu::new());
    let phase = cpu.local(SoftwarePhase::MainBeforeSubmit);
    let vector = IrqVector::new(64);
    let (report_tx, report_rx) = oneshot::channel();
    let delivery = spawn_delivery(DeliveryConfig {
        cpu: Arc::clone(&cpu),
        access: access.clone(),
        phase: phase.clone(),
        vector,
        report: report_tx,
    });

    let main_access = access.clone();
    let main_cpu = Arc::clone(&cpu);
    let main = tokio::spawn(async move {
        main_cpu
            .run_main(async move {
                let mut main_port = PdmaRegisterPort::new(main_access)?;
                let mut driver = AsyncDriver::setup_on(&mut main_port, 1, 4).await?;
                let queue = driver.queue_config_handle();
                let source_capability = driver.completion_irq_source();
                let source = source_capability.source();
                delivery
                    .bootstrap
                    .send((driver.clone(), main_port.clone()))
                    .map_err(|_| InterruptError::DeliveryUnavailable)?;
                delivery
                    .ready
                    .await
                    .map_err(|_| InterruptError::DeliveryUnavailable)?;
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
                    .map_err(|_| InterruptError::DeliveryUnavailable)?;
                let phase = phase.with(|phase| {
                    assert_eq!(*phase, SoftwarePhase::Handler);
                    *phase = SoftwarePhase::MainAfterHandler;
                    *phase
                })?;
                let copied = driver.read_buffer_on(&main_port, dst, 4).await?;
                let queue_status = main_port
                    .queue_status(EpochQueue::new(queue.epoch(), queue.queue_id()))
                    .await?;
                Ok::<_, Box<dyn std::error::Error + Send + Sync>>(MainOutcome {
                    command,
                    queue,
                    source,
                    report,
                    copied,
                    phase,
                    queue_status,
                })
            })
            .await
    });

    let outcome = main.await.expect("software main joins cleanly")?;
    delivery
        .shutdown
        .await
        .map_err(|_| InterruptError::DeliveryUnavailable)?
        .request_shutdown();
    let _ = delivery.join.send(());
    device.request_shutdown().await;
    let delivery_summary = delivery.task.await.expect("host delivery joins cleanly");
    let device_summary = worker.await.expect("device worker joins cleanly");
    let mmio_accesses = observer.snapshot();
    let memory_accesses = access.records();
    let queue_status = outcome.queue_status;
    assert_outcome(&ScenarioResult {
        vector,
        outcome,
        queue_status,
        deliveries: delivery_summary.deliveries(),
        device_steps: device_summary.steps,
        device_trace: handle.trace_events().await,
        mmio_accesses,
        memory_accesses,
    });
    Ok(())
}
